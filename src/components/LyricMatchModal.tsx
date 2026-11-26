import { useEffect, useState } from "react";
import { Check, Download, FileAudio, Loader2, RotateCcw, Search } from "lucide-react";
import Modal from "./Modal";
import { api } from "../api";
import { useStore } from "../store";
import { useI18n } from "../i18n";
import { fmtTime } from "../utils";

interface LyricCandidate {
  source: "netease" | "qq" | "kugou";
  rid: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  score: number;
}

interface LyricMatchModalProps {
  open: boolean;
  onClose: () => void;
  trackId: number;
  title: string;
  artist: string;
  durationMs: number;
}

export default function LyricMatchModal({
  open,
  onClose,
  trackId,
  title,
  artist,
  durationMs,
}: LyricMatchModalProps) {
  const { tr } = useI18n();
  const lyrics = useStore((s) => s.lyrics);
  const loadLyricsByKey = useStore((s) => s.loadLyricsByKey);
  const toast = useStore((s) => s.toast);

  const cleanArtist =
    artist === "未知艺术家" || artist.toLowerCase() === "unknown artist" ? "" : artist;
  const defaultKw = cleanArtist ? `${title} ${cleanArtist}` : title;

  const [kw, setKw] = useState(defaultKw);
  const [searching, setSearching] = useState(false);
  const [applyingKey, setApplyingKey] = useState<string | null>(null);
  const [exporting, setExporting] = useState<"sidecar" | "embed" | null>(null);
  const [cands, setCands] = useState<LyricCandidate[]>([]);
  const [searched, setSearched] = useState(false);

  const runSearch = async (query: string) => {
    const q = query.trim();
    if (!q) return;
    setSearching(true);
    setSearched(true);
    try {
      const list = await api.searchLocalLyricCandidates(q, title, cleanArtist, durationMs);
      setCands(list);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setSearching(false);
    }
  };

  useEffect(() => {
    if (!open) return;
    setKw(defaultKw);
    setCands([]);
    setSearched(false);
    void runSearch(defaultKw);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, trackId]);

  const reloadCurrentLyrics = async () => {
    useStore.setState({ lyricsFor: null });
    await loadLyricsByKey(`track-${trackId}`);
  };

  const handleApply = async (c: LyricCandidate) => {
    const key = `${c.source}-${c.rid}`;
    if (applyingKey) return;
    setApplyingKey(key);
    try {
      await api.applyLocalLyricCandidate(trackId, c.source, c.rid);
      await reloadCurrentLyrics();
      toast(tr("已应用匹配歌词（已缓存）", "Matched lyrics applied & cached"), "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setApplyingKey(null);
    }
  };

  const handleClear = async () => {
    try {
      await api.clearLocalLyricMatch(trackId);
      await reloadCurrentLyrics();
      toast(tr("已重置歌词匹配", "Lyric match reset"), "info");
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const handleExport = async (mode: "sidecar" | "embed") => {
    if (!lyrics || !lyrics.lines.length || exporting) return;
    setExporting(mode);
    try {
      await api.exportLocalLyric(trackId, mode, lyrics);
      toast(
        mode === "sidecar"
          ? tr("已保存为同目录 .lrc 文件", "Saved as sidecar .lrc file")
          : tr("已写入音频文件内嵌歌词标签", "Embedded lyrics into audio file"),
        "success"
      );
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setExporting(null);
    }
  };

  const sourceBadge = (src: LyricCandidate["source"]) => {
    if (src === "netease") return { text: tr("网易云", "NetEase"), cls: "text-[#e0533f] bg-[#e0533f]/12" };
    if (src === "qq") return { text: "QQ", cls: "text-[#22c55e] bg-[#22c55e]/12" };
    return { text: tr("酷狗", "KuGou"), cls: "text-[#3b82f6] bg-[#3b82f6]/12" };
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={tr("匹配本地歌词", "Match Local Lyrics")}
      width={520}
    >
      <div className="flex flex-col gap-3.5">
        {/* 搜索栏 */}
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
            />
            <input
              type="text"
              value={kw}
              onChange={(e) => setKw(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && runSearch(kw)}
              placeholder={tr("输入歌名或歌手搜索歌词…", "Search lyrics by title or artist…")}
              className="w-full h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] pl-8 pr-3 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none focus:border-[var(--accent)]"
            />
          </div>
          <button
            className="btn-primary !h-9 !px-3.5 shrink-0"
            disabled={searching || !kw.trim()}
            onClick={() => runSearch(kw)}
          >
            {searching ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
            {tr("搜索", "Search")}
          </button>
        </div>

        {/* 候选列表 */}
        <div className="max-h-[290px] overflow-y-auto flex flex-col gap-1 pr-0.5">
          {searching && (
            <div className="py-10 flex items-center justify-center gap-2 text-[12.5px] text-[var(--ink-3)]">
              <Loader2 size={15} className="animate-spin" />
              {tr("正在从网易云 / QQ / 酷狗搜索候选…", "Searching NetEase / QQ / KuGou…")}
            </div>
          )}
          {!searching && searched && cands.length === 0 && (
            <div className="py-10 text-center text-[12.5px] text-[var(--ink-3)]">
              {tr("未找到相关候选，请尝试简化关键词", "No candidates found. Try a shorter keyword.")}
            </div>
          )}
          {!searching &&
            cands.map((c) => {
              const key = `${c.source}-${c.rid}`;
              const badge = sourceBadge(c.source);
              const busy = applyingKey === key;
              return (
                <div
                  key={key}
                  className="flex items-center justify-between gap-3 px-3 py-2 rounded-xl hover:bg-[var(--shade-hover)] border border-transparent hover:border-[var(--line)] transition-colors"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span
                        className={`text-[10px] font-semibold px-1.5 py-0.5 rounded shrink-0 ${badge.cls}`}
                      >
                        {badge.text}
                      </span>
                      <span className="text-[13px] font-medium text-[var(--ink)] truncate">
                        {c.title}
                      </span>
                    </div>
                    <div className="text-[11.5px] text-[var(--ink-3)] truncate mt-0.5">
                      {c.artist}
                      {c.album ? ` · ${c.album}` : ""}
                      {c.durationMs > 0 ? ` · ${fmtTime(c.durationMs)}` : ""}
                    </div>
                  </div>
                  <button
                    className="btn-secondary !py-1 !px-2.5 !text-[11.5px] shrink-0"
                    disabled={!!applyingKey}
                    onClick={() => handleApply(c)}
                  >
                    {busy ? <Loader2 size={12} className="animate-spin" /> : <Check size={12} />}
                    {tr("应用", "Apply")}
                  </button>
                </div>
              );
            })}
        </div>

        {/* 底部：导出当前歌词与重置 */}
        <div className="pt-3 border-t border-[var(--line)] flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <button
              className="btn-secondary !py-1.5 !px-2.5 !text-[11.5px]"
              disabled={!lyrics?.lines.length || !!exporting}
              onClick={() => handleExport("sidecar")}
              title={tr("将当前歌词保存为歌曲同目录同名 .lrc 文件", "Save current lyrics as sidecar .lrc next to audio")}
            >
              {exporting === "sidecar" ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Download size={12} />
              )}
              {tr("保存同目录 .lrc", "Save .lrc")}
            </button>
            <button
              className="btn-secondary !py-1.5 !px-2.5 !text-[11.5px]"
              disabled={!lyrics?.lines.length || !!exporting}
              onClick={() => handleExport("embed")}
              title={tr("将当前歌词写入音频文件的内嵌标签（支持 MP3/FLAC/M4A/OGG）", "Embed current lyrics into audio tags")}
            >
              {exporting === "embed" ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <FileAudio size={12} />
              )}
              {tr("写入内嵌标签", "Embed Tag")}
            </button>
          </div>
          <button
            className="btn-ghost !px-2.5 !h-7 !text-[11.5px] text-[var(--ink-3)]"
            onClick={handleClear}
            title={tr("清除本曲匹配缓存并恢复默认查找", "Clear cached match and reload default")}
          >
            <RotateCcw size={12} />
            {tr("重置", "Reset")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
