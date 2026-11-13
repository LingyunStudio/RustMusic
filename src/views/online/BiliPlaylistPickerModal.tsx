//! 「添加到播放列表」弹窗：B 站结果区单行右键 / 批量多选共用。
import { useState } from "react";
import { useStore } from "../../store";
import { useI18n } from "../../i18n";
import Modal from "../../components/Modal";
import type { BiliSpaceItem } from "../../types";

export function BiliPlaylistPickerModal({
  rows,
  onClose,
  toRow,
}: {
  rows: BiliSpaceItem[] | null;
  onClose: () => void;
  toRow: (r: BiliSpaceItem) => any;
}) {
  const { tr } = useI18n();
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const addOnlineToPlaylist = useStore((s) => s.addOnlineToPlaylist);
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const [newPlName, setNewPlName] = useState("");
  // “创建并添加”进行中：双击会创建两个同名歌单（后端不按名去重）
  const [creatingPl, setCreatingPl] = useState(false);

  return (
    <Modal
      open={rows != null}
      onClose={onClose}
      title={
        rows && rows.length > 1
          ? tr(`添加 ${rows.length} 首到播放列表`, `Add ${rows.length} Tracks to Playlist`)
          : tr("添加到播放列表", "Add to Playlist")
      }
      width={380}
    >
      <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
        {playlists.map((p) => (
          <button
            key={p.id}
            className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
            onClick={async () => {
              if (rows) {
                for (const r of rows) {
                  await addOnlineToPlaylist(p.id, toRow(r));
                }
              }
              onClose();
            }}
          >
            <span className="truncate">{p.name}</span>
            <span className="text-[11px] text-[var(--ink-2)]">
              {tr(`${p.entries.length} 首`, `${p.entries.length} ${p.entries.length === 1 ? "track" : "tracks"}`)}
            </span>
          </button>
        ))}
        {!playlists.length && (
          <div className="text-[12.5px] text-[var(--ink-2)] py-2">
            {tr("还没有播放列表，在下方创建", "No playlists yet. Create one below.")}
          </div>
        )}
      </div>
      <div className="flex gap-2 mt-3">
        <input
          type="text"
          value={newPlName}
          onChange={(e) => setNewPlName(e.target.value)}
          placeholder={tr("新播放列表名称", "New playlist name")}
          className={
            isLiquid
              ? "liquid-input flex-1 h-9 rounded-lg px-3 text-[13px]"
              : "flex-1 h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3 text-[13px] focus:border-[var(--line)] outline-none"
          }
        />
        <button
          className="btn-secondary shrink-0 whitespace-nowrap"
          disabled={creatingPl}
          onClick={async () => {
            if (creatingPl || !newPlName.trim() || !rows?.length) return;
            setCreatingPl(true);
            try {
              const pid = await createPlaylist(newPlName.trim());
              if (pid >= 0) {
                for (const r of rows) {
                  await addOnlineToPlaylist(pid, toRow(r));
                }
              }
            } finally {
              setCreatingPl(false);
            }
            setNewPlName("");
            onClose();
          }}
        >
          {tr("创建并添加", "Create & Add")}
        </button>
      </div>
    </Modal>
  );
}
