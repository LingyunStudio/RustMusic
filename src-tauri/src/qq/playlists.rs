//! 用户歌单（需登录）。

use super::url::*;
use super::*;

pub(super) fn plain_get_with_cookie(url: &str, referer: &str, cookie: &str) -> Result<String, String> {
    http_agent()
        .get(url)
        .set("Referer", referer)
        .set("User-Agent", UA)
        .set("Cookie", cookie)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("QQ 音乐接口请求失败: {e}"))?
        .into_string()
        .map_err(|e| format!("QQ 音乐响应读取失败: {e}"))
}

/// 用户创建的歌单列表
pub fn user_playlists(
    musicid: &str,
    musickey: &str,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.musicasset.PlaylistBaseRead",
            "method": "GetPlaylistByUin",
            "param": {"uin": musicid}
        }
    });
    let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
    let code = resp
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    if code != 0 {
        return Err(if code == 104009 {
            "QQ 音乐登录已过期，请重新登录".into()
        } else {
            format!("获取歌单列表失败（code {code}）")
        });
    }
    let mut out = Vec::new();
    if let Some(list) = resp
        .pointer("/req_1/data/v_playlist")
        .and_then(|v| v.as_array())
    {
        for p in list {
            // 响应为驼峰命名：tid / dirName / songNum（蛇形为兼容备选）
            let id = p
                .get("tid")
                .or_else(|| p.get("dirId"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let name = p
                .get("dirName")
                .or_else(|| p.get("diss_name"))
                .or_else(|| p.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let count = p
                .get("songNum")
                .or_else(|| p.get("song_num"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            // “我喜欢/收藏”(dirId=201) 是 QQ 的特殊收藏夹：没有可用的
            // 普通 dissid，统一用 201 作哨兵 id 导出，playlist_tracks
            // 按 201 走收藏夹专用参数拉取
            let dir_id = p
                .get("dirId")
                .or_else(|| p.get("dirid"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let id = if dir_id == 201 { 201 } else { id };
            if id != 0 && !name.is_empty() {
                out.push(crate::models::UserPlaylistMeta {
                    id,
                    name,
                    track_count: count,
                });
            }
        }
    }
    Ok(out)
}

/// 加密 uin（CgiGetDiss 拉收藏夹需要）：登录响应里带原值 encryptUin，
/// 老登录态没存时按官方规则推导（3 个 NUL 前缀 + uin 的 base64）
pub fn encrypt_uin_of(musicid: &str, stored: &str) -> String {
    if !stored.is_empty() {
        return stored.to_string();
    }
    let mut buf = vec![0u8, 0, 0];
    buf.extend_from_slice(musicid.as_bytes());
    b64_encode(&buf)
}

/// 从 CgiGetDiss / 榜单详情的歌单条目解析 QqSong（mid/songmid 新旧字段兼容）
pub(super) fn song_from_diss_json(t: &serde_json::Value) -> Option<QqSong> {
    let mid = t
        .get("mid")
        .or_else(|| t.get("songmid"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if mid.is_empty() {
        return None;
    }
    let media_mid = t
        .get("media_mid")
        .and_then(|v| v.as_str())
        .or_else(|| t.pointer("/file/media_mid").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    Some(QqSong {
        id: mid.to_string(),
        name: t
            .get("name")
            .or_else(|| t.get("songname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        singer: t
            .get("singer")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                    .collect::<Vec<_>>()
                    .join(" / ")
            })
            .unwrap_or_default(),
        album: t
            .pointer("/album/name")
            .or_else(|| t.get("albumname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        album_mid: t
            .pointer("/album/mid")
            .or_else(|| t.get("albummid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        media_mid,
        duration_ms: t.get("interval").and_then(|v| v.as_i64()).unwrap_or(0) as u64 * 1000,
        vip: t
            .pointer("/pay/pay_play")
            .and_then(|v| v.as_i64())
            .map(|v| v != 0)
            .unwrap_or(false),
    })
}

/// 歌单内曲目（CgiGetDiss，分页拉全）。
/// disstid=201 为哨兵：拉取“我喜欢/收藏”夹（dirId=201，disstid 需传 0，
/// 并附 enc_host_uin），普通歌单照旧传真实 dissid。
pub fn playlist_tracks(
    disstid: i64,
    musicid: &str,
    musickey: &str,
    encrypt_uin: &str,
) -> Result<Vec<QqSong>, String> {
    let fav_mode = disstid == 201;
    let mut all = Vec::new();
    let mut begin = 0i64;
    let mut last_first_mid = String::new();
    loop {
        let param = if fav_mode {
            serde_json::json!({
                "disstid": 0,
                "dirid": 201,
                "tag": true,
                "song_begin": begin,
                "song_num": 100,
                "userinfo": false,
                "orderlist": true,
                "onlysonglist": 0,
                "enc_host_uin": encrypt_uin,
            })
        } else {
            serde_json::json!({
                "disstid": disstid,
                "dirid": 1,
                "tag": false,
                "song_begin": begin,
                "song_num": 100,
                "userinfo": false,
                "orderlist": true,
                "onlysonglist": 0
            })
        };
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.srfDissInfo.DissInfo",
                "method": "CgiGetDiss",
                "param": param
            }
        });
        let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
        let code = resp
            .pointer("/req_1/code")
            .and_then(|c| c.as_i64())
            .unwrap_or(0);
        if code != 0 {
            return Err(if code == 104009 {
                "QQ 音乐登录已过期，请重新登录".into()
            } else {
                format!("获取歌单详情失败（code {code}）")
            });
        }
        let list = resp
            .pointer("/req_1/data/songlist")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for t in &list {
            if let Some(s) = song_from_diss_json(t) {
                all.push(s);
            }
        }
        if got < 100 || begin > 10000 {
            break;
        }
        // 防呆：接口在某个深度开始重复返回同一页（不再前进）时立即停，
        // 避免把同一批歌重复写入导入结果
        let first_mid = list
            .first()
            .and_then(|t| {
                t.get("mid")
                    .or_else(|| t.get("songmid"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("")
            .to_string();
        if !first_mid.is_empty() && first_mid == last_first_mid {
            break;
        }
        last_first_mid = first_mid;
        begin += 100;
    }
    Ok(all)
}
