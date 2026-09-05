import { useMemo, useState } from "react";
import { Link2, Play, Plus, Radio, Trash2 } from "lucide-react";
import { useStore } from "../store";
import { fmtDate } from "../utils";

export default function SourcesView() {
  const sources = useStore((s) => s.sources);
  const addSource = useStore((s) => s.addSource);
  const deleteSource = useStore((s) => s.deleteSource);
  const playSourceItem = useStore((s) => s.playSourceItem);
  const current = useStore((s) => s.current);
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");

  const canAdd = useMemo(() => /^https?:\/\//.test(url.trim()), [url]);

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="pt-5 pb-3 px-5">
        <h1 className="text-[22px] font-bold flex items-center gap-2.5">
          <Radio size={20} className="text-[var(--dyn)]" />
          在线音源
          <span className="text-[13px] font-normal text-zinc-500 mb-0.5">{sources.length} 个</span>
        </h1>
        <p className="text-[12.5px] text-zinc-500 mt-1.5 leading-relaxed">
          添加任意音频文件直链（http/https 的 mp3 / flac / wav / ogg / m4a 等），
          RustMusic 会自动下载缓存后播放。请仅添加你有权使用的音源。
        </p>

        <div className="flex gap-2 mt-3.5">
          <div className="relative flex-1 max-w-[480px]">
            <Link2
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
            />
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && canAdd) {
                  addSource(url.trim(), title.trim());
                  setUrl("");
                  setTitle("");
                }
              }}
              placeholder="https://example.com/song.mp3"
              className="w-full h-9 rounded-lg bg-white/[0.06] border border-white/[0.09] pl-8 pr-3 text-[12.5px] focus:border-white/25 transition-colors"
            />
          </div>
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && canAdd) {
                addSource(url.trim(), title.trim());
                setUrl("");
                setTitle("");
              }
            }}
            placeholder="名称（可选）"
            className="w-[160px] h-9 rounded-lg bg-white/[0.06] border border-white/[0.09] px-3 text-[12.5px] focus:border-white/25 transition-colors"
          />
          <button
            className="btn-primary"
            disabled={!canAdd}
            style={{ opacity: canAdd ? 1 : 0.45 }}
            onClick={() => {
              addSource(url.trim(), title.trim());
              setUrl("");
              setTitle("");
            }}
          >
            <Plus size={14} />
            添加
          </button>
        </div>
      </header>

      <div className="flex-1 min-h-0 overflow-y-auto px-5 pb-4">
        {sources.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center gap-3 text-zinc-600">
            <div
              className="w-16 h-16 rounded-2xl flex items-center justify-center"
              style={{
                background: "rgba(255,255,255,0.04)",
                border: "1px solid rgba(255,255,255,0.06)",
              }}
            >
              <Radio size={26} className="text-zinc-600" />
            </div>
            <div className="text-[13.5px]">还没有添加任何在线音源</div>
          </div>
        ) : (
          <div className="flex flex-col gap-1.5">
            {sources.map((s) => {
              const active =
                current?.kind === "url" && current.path.includes(s.url.slice(-20));
              return (
                <div
                  key={s.id}
                  className="group flex items-center gap-3 h-[58px] px-3.5 rounded-xl bg-white/[0.03] border border-white/[0.05] hover:bg-white/[0.06] transition-colors"
                >
                  <button
                    className="w-9 h-9 rounded-full flex items-center justify-center shrink-0 transition-transform hover:scale-105"
                    style={{ background: "linear-gradient(135deg, #6366f1, #8b5cf6)" }}
                    onClick={() => playSourceItem(s)}
                  >
                    <Play size={14} className="fill-current text-white ml-px" />
                  </button>
                  <div className="min-w-0 flex-1">
                    <div className="text-[13px] text-zinc-100 truncate">
                      {s.title || s.url.split("/").pop() || "在线音源"}
                    </div>
                    <div className="text-[11.5px] text-zinc-500 truncate">{s.url}</div>
                  </div>
                  <span className="text-[11px] text-zinc-600 shrink-0">
                    {fmtDate(s.createdAt)}
                  </span>
                  <button
                    className="btn-ghost w-7 h-7 shrink-0 opacity-0 group-hover:opacity-100 hover:!text-rose-400"
                    onClick={() => deleteSource(s.id)}
                    title="删除"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
