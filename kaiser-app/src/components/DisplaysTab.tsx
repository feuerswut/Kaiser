import { useState } from "react";
import { toast } from "sonner";
import { RefreshCw, Power, Monitor } from "lucide-react";
import { api } from "../api";
import type { DisplayInfo, SnapshotDto } from "../types";

interface Props {
  snapshot: SnapshotDto;
  onRefresh: () => Promise<void>;
}

export function DisplaysTab({ snapshot, onRefresh }: Props) {
  const [busy, setBusy] = useState<string | null>(null);

  async function toggleDisplay(display: DisplayInfo) {
    const key = `${display.id.adapter_luid}:${display.id.target_id}`;
    setBusy(key);
    try {
      await api.toggleDisplay(display.id);
      await onRefresh();
      toast.success(
        `${display.friendly_name} ${display.is_active ? "disabled" : "enabled"}`
      );
    } catch (err) {
      toast.error(`Toggle failed: ${err}`);
    } finally {
      setBusy(null);
    }
  }

  const { displays, layout, pending_confirmation, pending_confirmation_remaining_secs } = snapshot;

  return (
    <div className="space-y-4">
      {pending_confirmation && (
        <div className="rounded-lg border border-yellow-600 bg-yellow-950/40 px-4 py-3 text-sm flex items-center justify-between">
          <span className="text-yellow-300">
            Layout change pending confirmation (
            {pending_confirmation_remaining_secs != null
              ? `${Math.ceil(pending_confirmation_remaining_secs)}s`
              : "…"}
            )
          </span>
          <button
            onClick={onRefresh}
            className="text-xs text-yellow-400 hover:text-yellow-200 underline"
          >
            Refresh
          </button>
        </div>
      )}

      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-zinc-400">
          Connected Displays
        </h2>
        <button
          onClick={onRefresh}
          className="flex items-center gap-1 text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
        >
          <RefreshCw size={12} />
          Refresh
        </button>
      </div>

      <div className="grid gap-3">
        {displays.map((display) => {
          const key = `${display.id.adapter_luid}:${display.id.target_id}`;
          const output = layout.outputs.find(
            (o) =>
              o.display_id.adapter_luid === display.id.adapter_luid &&
              o.display_id.target_id === display.id.target_id
          );

          return (
            <div
              key={key}
              className={`rounded-lg border p-4 transition-colors ${
                display.is_active
                  ? "border-zinc-700 bg-zinc-900"
                  : "border-zinc-800 bg-zinc-900/50 opacity-60"
              }`}
            >
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <Monitor
                    size={20}
                    className={
                      display.is_active ? "text-blue-400" : "text-zinc-600"
                    }
                  />
                  <div>
                    <div className="font-medium text-sm">
                      {display.friendly_name}
                    </div>
                    <div className="text-xs text-zinc-500 mt-0.5">
                      {display.resolution.width}×{display.resolution.height} @{" "}
                      {Math.round(display.refresh_rate_mhz / 1000)} Hz
                      {display.is_primary && (
                        <span className="ml-2 text-blue-400">Primary</span>
                      )}
                    </div>
                    {output && display.is_active && (
                      <div className="text-xs text-zinc-600 mt-0.5">
                        Position: ({output.position.x}, {output.position.y})
                      </div>
                    )}
                  </div>
                </div>

                <button
                  onClick={() => toggleDisplay(display)}
                  disabled={busy === key}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium transition-colors ${
                    display.is_active
                      ? "bg-red-900/40 text-red-400 hover:bg-red-900/60 border border-red-900"
                      : "bg-green-900/40 text-green-400 hover:bg-green-900/60 border border-green-900"
                  } disabled:opacity-50`}
                >
                  <Power size={12} />
                  {busy === key
                    ? "…"
                    : display.is_active
                    ? "Disable"
                    : "Enable"}
                </button>
              </div>
            </div>
          );
        })}

        {displays.length === 0 && (
          <div className="text-center text-zinc-500 py-12 text-sm">
            No displays detected
          </div>
        )}
      </div>
    </div>
  );
}
