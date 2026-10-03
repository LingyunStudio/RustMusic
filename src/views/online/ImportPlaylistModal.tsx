//! 导入歌单弹窗：拉取账号下歌单后单个导入或全部导入（带进度）。
import { useState } from "react";
import { ListPlus, Loader2 } from "lucide-react";
import { useStore } from "../../store";
import Modal from "../../components/Modal";
import type { Source } from "./shared";

export function ImportPlaylistModal({
  open,
  onClose,
  list,
  source,
  sourceName,
}: {
  open: boolean;
  onClose: () => void;
  list: { id: number | string; name: string; trackCount: number }[] | null;
  source: Source;
  sourceName: string;
}) {
  const importNeteasePlaylist = useStore((s) => s.importNeteasePlaylist);
  const importQqPlaylist = useStore((s) => s.importQqPlaylist);
  const importKugouPlaylist = useStore((s) => s.importKugouPlaylist);
  const importAllPlaylists = useStore((s) => s.importAllPlaylists);
  const [importing, setImporting] = useState(false);
  const [importProgress, setImportProgress] = useState<{
    done: number;
    total: number;
    name: string;
  } | null>(null);

  return (
      <Modal
        open={open}
        onClose={onClose}
        title={`导入${sourceName}歌单`}
        width={400}
      >
        {list === null ? (
          <div className="flex items-center justify-center gap-2 text-[13px] text-[var(--ink-2)] py-6">
            <Loader2 size={15} className="animate-spin" /> 获取中…
          </div>
        ) : (
          <>
            {list.length > 1 && (
              <>
                {/* 与列表行同构的低调样式：accent 图标+文字，右侧计数，行下加分隔线 */}
                <button
                  disabled={importing}
                  className="h-10 px-3 rounded-lg text-left text-[13px] hover:bg-[var(--shade)] flex items-center justify-between transition-colors disabled:opacity-60"
                  onClick={async () => {
                    if (importing || !list.length) return;
                    setImporting(true);
                    // 逐个导入，弹窗内实时显示进度；结束后由 store 统一
                    // 刷新列表并汇总提示（含失败个数）
                    await importAllPlaylists(source, list, setImportProgress);
                    setImporting(false);
                    setImportProgress(null);
                    onClose();
                  }}
                >
                  {importProgress ? (
                    <span className="flex items-center gap-2 text-[var(--ink-2)] min-w-0">
                      <Loader2 size={14} className="animate-spin shrink-0 text-[var(--accent)]" />
                      <span className="truncate">
                        正在导入 {importProgress.done + 1}/{importProgress.total}
                        ：「{importProgress.name}」
                      </span>
                    </span>
                  ) : (
                    <>
                      <span className="flex items-center gap-2 text-[var(--accent)] font-medium">
                        <ListPlus size={15} />
                        全部导入
                      </span>
                      <span className="text-[11px] text-[var(--ink-3)] shrink-0 ml-3">
                        共 {list.length} 个歌单
                      </span>
                    </>
                  )}
                </button>
                <div className="border-b border-[var(--line)] mb-2" aria-hidden />
              </>
            )}
            <div className="flex flex-col gap-1.5 max-h-[320px] overflow-y-auto">
              {list.map((p) => (
                <button
                  key={p.id}
                  disabled={importing}
                  className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors disabled:opacity-50"
                  onClick={async () => {
                    setImporting(true);
                    if (source === "netease") await importNeteasePlaylist(p.id as number, p.name);
                    else if (source === "qq") await importQqPlaylist(p.id as number, p.name);
                    else await importKugouPlaylist(String(p.id), p.name);
                    setImporting(false);
                    onClose();
                  }}
                >
                  <span className="truncate">{p.name}</span>
                  <span className="text-[11px] text-[var(--ink-2)] shrink-0 ml-3">
                    {p.trackCount} 首
                  </span>
                </button>
              ))}
              {!list.length && (
                <div className="text-[12.5px] text-[var(--ink-2)] py-2">账号下没有歌单</div>
              )}
            </div>
          </>
        )}
        <div className="text-[10.5px] text-[var(--ink-3)] mt-3 leading-relaxed">
          导入的歌单以在线条目保存：播放时按账号权益实时获取播放链接，不占用本地磁盘。
        </div>
      </Modal>
  );
}
