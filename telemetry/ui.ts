import React, { useMemo, useRef, useState } from "react";

function parseCSV(csv) {
  const [headerLine, ...lines] = csv.trim().split("\n");
  const headers = headerLine.split(",");

  return lines.map(line => {
    const values = line.split(",");
    const obj = {};
    headers.forEach((h, i) => (obj[h] = Number(values[i])));
    return obj;
  });
}

function groupData(rows) {
  const smMap = new Map();

  for (const r of rows) {
    if (!smMap.has(r.sm_id)) smMap.set(r.sm_id, new Map());
    const warpMap = smMap.get(r.sm_id);

    if (!warpMap.has(r.warp_id)) {
      warpMap.set(r.warp_id, {
        warp_state: r.warp_state,
        threads: []
      });
    }

    warpMap.get(r.warp_id).threads.push(r);
  }

  return smMap;
}

function colorForWarp(warpId) {
  const colors = ["#60a5fa", "#34d399", "#fbbf24", "#f87171", "#a78bfa"];
  return colors[warpId % colors.length];
}

export default function GPUVisualizer() {
  const [rows, setRows] = useState([]);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [focusedSm, setFocusedSm] = useState(null);
  const dragRef = useRef(null);

  const data = useMemo(() => groupData(rows), [rows]);

  const handleFileUpload = (e) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
      const text = event.target.result;
      const parsed = parseCSV(text);
      setRows(parsed);
    };
    reader.readAsText(file);
  };

  const onWheel = (e) => {
    e.preventDefault();
    const delta = -e.deltaY * 0.001;
    setScale((s) => Math.min(4, Math.max(0.3, s + delta)));
  };

  const onMouseDown = (e) => {
    dragRef.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
  };

  const onMouseMove = (e) => {
    if (!dragRef.current) return;
    const dx = e.clientX - dragRef.current.x;
    const dy = e.clientY - dragRef.current.y;
    setOffset({
      x: dragRef.current.ox + dx,
      y: dragRef.current.oy + dy
    });
  };

  const onMouseUp = () => {
    dragRef.current = null;
  };

  const showCoords = scale > 2.2;

  const smEntries = [...data.entries()];
  const visibleSMs = focusedSm !== null
    ? smEntries.filter(([id]) => id === focusedSm)
    : smEntries;

  const smGridCols = Math.ceil(Math.sqrt(smEntries.length || 1));

  return (
    <div className="p-6 bg-gray-950 min-h-screen text-white overflow-hidden">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-bold">
          GPU SM / Warp / Thread Visualizer
        </h1>

        <div className="flex gap-2 items-center">
          {focusedSm !== null && (
            <button
              onClick={() => setFocusedSm(null)}
              className="bg-gray-800 border border-gray-700 px-3 py-2 rounded"
            >
              Back to All SMs
            </button>
          )}

          <input
            type="file"
            accept=".csv"
            onChange={handleFileUpload}
            className="text-sm bg-gray-800 border border-gray-700 rounded px-3 py-2"
          />
        </div>
      </div>

      <div
        className="border border-gray-800 rounded-xl overflow-hidden"
        onWheel={onWheel}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onMouseDown={onMouseDown}
        style={{ cursor: dragRef.current ? "grabbing" : "grab" }}
      >
        <div
          style={{
            transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
            transformOrigin: "0 0"
          }}
          className="p-6"
        >
          <div
            className="grid gap-4"
            style={{ gridTemplateColumns: `repeat(${focusedSm ? 1 : smGridCols}, minmax(0, 1fr))` }}
          >
            {visibleSMs.map(([smId, warps]) => (
              <div
                key={smId}
                onClick={() => setFocusedSm(smId)}
                className="border border-gray-700 rounded-xl p-4 bg-gray-900 cursor-pointer hover:ring-2 hover:ring-blue-500"
              >
                <div className="text-lg font-semibold mb-3 text-gray-200">
                  SM {smId}
                </div>

                <div className="grid grid-cols-2 gap-2">
                  {[...warps.entries()].map(([warpId, warp]) => {
                    const threads = warp.threads;
                    const cols = 8;
                    const size = 200;
                    const cell = size / cols;

                    return (
                      <div
                        key={warpId}
                        className="rounded-lg p-2 bg-gray-950"
                        style={{ borderLeft: `6px solid ${colorForWarp(warpId)}` }}
                      >
                        <div className="text-xs text-gray-300 mb-2">
                          Warp {warpId}
                        </div>

                        <div
                          className="relative bg-gray-900 border border-gray-800 rounded-md"
                          style={{ width: size, height: size }}
                        >
                          {threads.slice(0, 32).map((t, i) => {
                            const x = (i % cols) * cell;
                            const y = Math.floor(i / cols) * cell;

                            return (
                              <div
                                key={t.thread_id}
                                className="absolute w-6 h-6 flex flex-col items-center justify-center rounded bg-gray-700 border border-gray-500 text-[10px]"
                                style={{ left: x, top: y }}
                                title={`thread ${t.thread_id} (${t.thread_x},${t.thread_y},${t.thread_z})`}
                              >
                                <div>{t.thread_id}</div>

                                {showCoords && (
                                  <div className="text-[8px] text-gray-300 leading-none">
                                    {t.thread_x},{t.thread_y},{t.thread_z}
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
