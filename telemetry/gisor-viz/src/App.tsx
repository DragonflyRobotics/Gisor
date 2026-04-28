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
  const dragRef = useRef(null);
  const touchRef = useRef({});

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
    setOffset({ x: dragRef.current.ox + dx, y: dragRef.current.oy + dy });
  };

  const onMouseUp = () => {
    dragRef.current = null;
  };

  // touch: 2-finger pan, 3-finger zoom
  const onTouchStart = (e) => {
    const t = e.touches;

    if (t.length === 2) {
      touchRef.current.mode = "pan";
      touchRef.current.startX = (t[0].clientX + t[1].clientX) / 2;
      touchRef.current.startY = (t[0].clientY + t[1].clientY) / 2;
      touchRef.current.startOffset = { ...offset };
    }

    if (t.length === 3) {
      touchRef.current.mode = "zoom";
      const dx = t[0].clientX - t[1].clientX;
      const dy = t[0].clientY - t[1].clientY;
      touchRef.current.startDist = Math.sqrt(dx * dx + dy * dy);
      touchRef.current.startScale = scale;
    }
  };

  const onTouchMove = (e) => {
    const t = e.touches;

    if (t.length === 2 && touchRef.current.mode === "pan") {
      const cx = (t[0].clientX + t[1].clientX) / 2;
      const cy = (t[0].clientY + t[1].clientY) / 2;

      const dx = cx - touchRef.current.startX;
      const dy = cy - touchRef.current.startY;

      setOffset({
        x: touchRef.current.startOffset.x + dx,
        y: touchRef.current.startOffset.y + dy
      });
    }

    if (t.length === 3 && touchRef.current.mode === "zoom") {
      const dx = t[0].clientX - t[1].clientX;
      const dy = t[0].clientY - t[1].clientY;
      const dist = Math.sqrt(dx * dx + dy * dy);
      const ratio = dist / touchRef.current.startDist;

      setScale(Math.min(5, Math.max(0.3, touchRef.current.startScale * ratio)));
    }
  };

  const onTouchEnd = () => {
    touchRef.current = {};
  };

  const showCoords = scale > 2.2;

  return (
    <div className="p-6 bg-gray-950 min-h-screen text-white overflow-hidden">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-bold">GPU SM / Warp / Thread Visualizer</h1>
        <input type="file" accept=".csv" onChange={handleFileUpload} className="text-sm bg-gray-800 border border-gray-700 rounded px-3 py-2" />
      </div>

      <div
        className="border border-gray-800 rounded-xl overflow-hidden"
        onWheel={onWheel}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
        style={{ touchAction: "none", cursor: dragRef.current ? "grabbing" : "grab" }}
      >
        <div
          style={{ transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`, transformOrigin: "0 0" }}
          className="p-6"
        >
          {[...data.entries()].map(([smId, warps]) => (
            <div key={smId} className="border border-gray-700 rounded-xl p-4 bg-gray-900 mb-6">
              <div className="text-lg font-semibold mb-3 text-gray-200">SM {smId}</div>

              <div className="grid grid-cols-2 gap-2">
                {[...warps.entries()].map(([warpId, warp]) => {
                  const cols = 8;
                  const size = 200;
                  const cell = size / cols;

                  return (
                    <div key={warpId} className="bg-gray-950 p-2 rounded-lg" style={{ borderLeft: `6px solid ${colorForWarp(warpId)}` }}>
                      <div className="text-xs text-gray-300 mb-2">Warp {warpId}</div>

                      <div className="relative bg-gray-900 border border-gray-800 rounded-md" style={{ width: size, height: size }}>
                        {warp.threads.slice(0, 32).map((t, i) => (
                          <div
                            key={t.thread_id}
                            className="absolute w-6 h-6 flex flex-col items-center justify-center text-[10px] bg-gray-700 border border-gray-500 rounded"
                            style={{ left: (i % cols) * cell, top: Math.floor(i / cols) * cell }}
                          >
                            <div>{t.thread_id}</div>
                            {showCoords && (
                              <div className="text-[8px] leading-none">
                                {t.thread_x},{t.thread_y},{t.thread_z}
                              </div>
                            )}
                          </div>
                        ))}
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
  );
}
