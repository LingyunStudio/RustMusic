//! 榜单 / 歌单广场 / 随机推荐（匿名可用）。

use super::playlists::*;
use super::*;

// ---------- 榜单 / 歌单广场 / 随机推荐（匿名可用） ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QqToplist {
    pub id: i64,
    pub title: String,
    pub pic: String,
    pub update_time: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QqPublicPlaylist {
    pub id: i64,
    pub name: String,
    pub cover: String,
    pub listen_num: i64,
    pub creator: String,
    pub songs: Vec<QqSong>,
}

/// 无签名 musicu 请求：榜单 / 公开歌单等匿名开放模块走本通道
///（实测匿名可访问，带登录 cookie 反而可能触发风控）
fn musicu_plain(payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let resp = http_agent()
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .set("Content-Type", "application/json")
        .set("Referer", "https://y.qq.com/")
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .send_string(&body)
        .map_err(|e| format!("QQ 音乐接口请求失败: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("QQ 音乐响应读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("QQ 音乐响应解析失败: {e}"))
}

fn require_module_ok(resp: &serde_json::Value, what: &str) -> Result<(), String> {
    let code = resp
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(-1);
    if code != 0 {
        return Err(format!("获取{what}失败（code {code}）"));
    }
    Ok(())
}

/// 官方榜单列表（巅峰榜/飙升榜/热歌榜等，匿名可用）
pub fn toplists() -> Result<Vec<QqToplist>, String> {
    let payload = serde_json::json!({
        "comm": {"ct": 24, "cv": 0},
        "req_1": {
            "module": "music.musicToplist.Toplist",
            "method": "GetAll",
            "param": {}
        }
    });
    let resp = musicu_plain(&payload)?;
    require_module_ok(&resp, "榜单列表")?;
    let mut out = Vec::new();
    if let Some(groups) = resp.pointer("/req_1/data/group").and_then(|v| v.as_array()) {
        for g in groups {
            let Some(tops) = g.get("toplist").and_then(|v| v.as_array()) else {
                continue;
            };
            for t in tops {
                let id = t.get("topId").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = t
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if id == 0 || title.is_empty() {
                    continue;
                }
                let pic = t
                    .get("headPicUrl")
                    .or_else(|| t.get("frontPicUrl"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let update_time = match t.get("updateTime") {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    _ => String::new(),
                };
                out.push(QqToplist {
                    id,
                    title,
                    pic,
                    update_time,
                });
            }
        }
    }
    Ok(out)
}

/// 榜单曲目（匿名可用），返回前 100 首
pub fn toplist_tracks(top_id: i64, page: i64) -> Result<Vec<QqSong>, String> {
    let num = 100i64;
    let offset = (page.max(1) - 1) * num;
    let payload = serde_json::json!({
        "comm": {"ct": 24, "cv": 0},
        "req_1": {
            "module": "music.musicToplist.Toplist",
            "method": "GetDetail",
            "param": {"topId": top_id, "offset": offset, "num": num}
        }
    });
    let resp = musicu_plain(&payload)?;
    require_module_ok(&resp, "榜单详情")?;
    let songs = resp
        .pointer("/req_1/data/songInfoList")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(song_from_diss_json).collect())
        .unwrap_or_default();
    Ok(songs)
}

/// 公开歌单详情（CgiGetDiss 匿名版；与 playlist_tracks 的登录版同源接口，
/// 公开歌单无需登录）。最多拉 1000 首，防止超大歌单拖慢“随便听听”。
pub fn public_playlist_tracks(disstid: i64) -> Result<QqPublicPlaylist, String> {
    let mut songs = Vec::new();
    let mut name = String::new();
    let mut cover = String::new();
    let mut listen_num = 0i64;
    let mut creator = String::new();
    let mut begin = 0i64;
    loop {
        let payload = serde_json::json!({
            "comm": {"ct": 24, "cv": 0},
            "req_1": {
                "module": "music.srfDissInfo.DissInfo",
                "method": "CgiGetDiss",
                "param": {
                    "disstid": disstid,
                    "dirid": 1,
                    "tag": false,
                    "song_begin": begin,
                    "song_num": 100,
                    "userinfo": false,
                    "orderlist": true,
                    "onlysonglist": 0
                }
            }
        });
        let resp = musicu_plain(&payload)?;
        require_module_ok(&resp, "歌单详情")?;
        if name.is_empty() {
            let dir = resp.pointer("/req_1/data/dirinfo");
            name = dir
                .and_then(|d| d.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            cover = dir
                .and_then(|d| d.get("picurl"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            listen_num = dir
                .and_then(|d| d.get("listennum"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            creator = dir
                .and_then(|d| d.pointer("/creator/name").or_else(|| d.get("host_nic")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
        }
        let list = resp
            .pointer("/req_1/data/songlist")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for t in &list {
            if let Some(s) = song_from_diss_json(t) {
                songs.push(s);
            }
        }
        if got < 100 || begin > 900 {
            break;
        }
        begin += 100;
    }
    Ok(QqPublicPlaylist {
        id: disstid,
        name,
        cover,
        listen_num,
        creator,
        songs,
    })
}

/// 歌单广场（匿名）：热门分类按偏移取页，返回 (分类下歌单总数, 摘要列表)
fn plaza_page(sin: i64, ein: i64) -> Result<(i64, Vec<QqPublicPlaylist>), String> {
    let url = format!(
        "https://c.y.qq.com/splcloud/fcgi-bin/fcg_get_diss_by_tag.fcg?picmid=1&rnd={}&g_tk=&loginUin=&hostUin=&format=json&inCharset=utf8&outCharset=utf-8&notice=0&platform=yqq.json&needNewCode=0&categoryId=10000000&sortId=5&sin={sin}&ein={ein}",
        chrono_now_ms()
    );
    let text = plain_get(&url, "https://y.qq.com/")?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("歌单广场解析失败: {e}"))?;
    if resp.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return Err(format!(
            "歌单广场请求失败（code {}）",
            resp.get("code").and_then(|c| c.as_i64()).unwrap_or(-1)
        ));
    }
    let sum = resp
        .pointer("/data/sum")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let mut out = Vec::new();
    if let Some(list) = resp.pointer("/data/list").and_then(|v| v.as_array()) {
        for p in list {
            let id = p
                .get("dissid")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let name = p
                .get("dissname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id == 0 || name.is_empty() {
                continue;
            }
            out.push(QqPublicPlaylist {
                id,
                name,
                cover: p
                    .get("imgurl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                listen_num: p.get("listennum").and_then(|v| v.as_i64()).unwrap_or(0),
                creator: p
                    .pointer("/creator/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                songs: vec![],
            });
        }
    }
    Ok((sum, out))
}

/// 随机公开歌单：热门分类下随机抽一个歌单并拉取曲目。
/// 个别歌单匿名拉取失败（隐私限制）或为空时自动换一个重试。
pub fn random_playlist() -> Result<QqPublicPlaylist, String> {
    use rand::Rng;
    let (sum, _) = plaza_page(0, 0)?;
    if sum <= 0 {
        return Err("歌单广场暂无数据".into());
    }
    let mut last_err = String::from("未能抽中可用歌单");
    for _ in 0..4 {
        let mut rng = rand::thread_rng();
        let sin = rng.gen_range(0..sum);
        let (_, page) = plaza_page(sin, sin)?;
        let Some(pick) = page.into_iter().next() else {
            continue;
        };
        match public_playlist_tracks(pick.id) {
            Ok(mut pl) => {
                if pl.cover.is_empty() {
                    pl.cover = pick.cover;
                }
                if pl.creator.is_empty() {
                    pl.creator = pick.creator;
                }
                pl.listen_num = pl.listen_num.max(pick.listen_num);
                if !pl.songs.is_empty() {
                    return Ok(pl);
                }
                last_err = "抽中的歌单暂无曲目".into();
            }
            Err(e) => last_err = e,
        }
    }
    Err(format!("随机歌单获取失败：{last_err}"))
}
