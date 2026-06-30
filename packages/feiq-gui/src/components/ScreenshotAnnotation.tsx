import { useRef, useState, useEffect, useCallback } from "react";
import {
  Square,
  ArrowRight,
  Type,
  Pen,
  Palette,
  Undo,
  Check,
  X,
} from "lucide-react";
import { writeFile } from "@tauri-apps/plugin-fs";

type Tool = "rect" | "arrow" | "text" | "free";

interface Point {
  x: number;
  y: number;
}

interface Annotation {
  type: Tool;
  color: string;
  points: Point[];
  text?: string;
}

interface TextInputPos {
  canvasPoint: Point;
  /** CSS-pixel offset relative to the canvas container */
  left: number;
  top: number;
}

const COLORS = ["#FF0000", "#0066FF", "#00CC00", "#FFDD00", "#FFFFFF", "#000000"];
const COLOR_LABELS = ["Red", "Blue", "Green", "Yellow", "White", "Black"];

/* ───────── helper: render one annotation onto a canvas context ───────── */

function drawAnnotation(ctx: CanvasRenderingContext2D, ann: Annotation) {
  ctx.save();
  ctx.strokeStyle = ann.color;
  ctx.fillStyle = ann.color;
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  switch (ann.type) {
    case "rect": {
      if (ann.points.length < 2) break;
      const [a, b] = ann.points;
      const x = Math.min(a.x, b.x);
      const y = Math.min(a.y, b.y);
      ctx.strokeRect(x, y, Math.abs(b.x - a.x), Math.abs(b.y - a.y));
      break;
    }

    case "arrow": {
      if (ann.points.length < 2) break;
      const [start, end] = ann.points;
      ctx.beginPath();
      ctx.moveTo(start.x, start.y);
      ctx.lineTo(end.x, end.y);
      ctx.stroke();

      const angle = Math.atan2(end.y - start.y, end.x - start.x);
      const head = 12;
      ctx.beginPath();
      ctx.moveTo(end.x, end.y);
      ctx.lineTo(
        end.x - head * Math.cos(angle - Math.PI / 6),
        end.y - head * Math.sin(angle - Math.PI / 6),
      );
      ctx.lineTo(
        end.x - head * Math.cos(angle + Math.PI / 6),
        end.y - head * Math.sin(angle + Math.PI / 6),
      );
      ctx.closePath();
      ctx.fill();
      break;
    }

    case "free": {
      if (ann.points.length < 2) break;
      ctx.beginPath();
      ctx.moveTo(ann.points[0].x, ann.points[0].y);
      for (let i = 1; i < ann.points.length; i++) {
        ctx.lineTo(ann.points[i].x, ann.points[i].y);
      }
      ctx.stroke();
      break;
    }

    case "text": {
      if (!ann.text || ann.points.length < 1) break;
      const p = ann.points[0];
      ctx.font = "bold 20px sans-serif";
      ctx.textBaseline = "top";
      ctx.fillText(ann.text, p.x, p.y);
      break;
    }
  }

  ctx.restore();
}

/* ───────── Component ───────── */

export function ScreenshotAnnotation({
  imagePath,
  onSave,
  onCancel,
}: {
  imagePath: string;
  onSave: (path: string) => void;
  onCancel: () => void;
}) {
  const bgCanvasRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const [tool, setTool] = useState<Tool>("rect");
  const [color, setColor] = useState(COLORS[0]);
  const [showPalette, setShowPalette] = useState(false);
  const [annotations, setAnnotations] = useState<Annotation[]>([]);
  const [imageLoaded, setImageLoaded] = useState(false);
  const [textInput, setTextInput] = useState<TextInputPos | null>(null);

  /* ref-based drawing state (avoids stale closures in mousemove) */
  const drawingRef = useRef(false);
  const annotationsRef = useRef<Annotation[]>([]);
  const drawingAnnRef = useRef<Annotation | null>(null);

  const syncAnnotations = useCallback((list: Annotation[]) => {
    annotationsRef.current = list;
    setAnnotations(list);
  }, []);

  /* ── load image into background canvas ── */
  useEffect(() => {
    const img = new Image();
    img.onload = () => {
      const nw = img.naturalWidth;
      const nh = img.naturalHeight;

      const bg = bgCanvasRef.current!;
      bg.width = nw;
      bg.height = nh;
      bg.getContext("2d")!.drawImage(img, 0, 0);

      const ov = overlayRef.current!;
      ov.width = nw;
      ov.height = nh;

      /* compute CSS display size to fit viewport */
      const toolbarH = 50;
      const pad = 20;
      const mw = innerWidth - pad * 2;
      const mh = innerHeight - toolbarH - pad * 2;
      let dw = nw;
      let dh = nh;
      if (dw > mw) {
        dh = (dh / dw) * mw;
        dw = mw;
      }
      if (dh > mh) {
        dw = (dw / dh) * mh;
        dh = mh;
      }

      bg.style.width = `${dw}px`;
      bg.style.height = `${dh}px`;
      ov.style.width = `${dw}px`;
      ov.style.height = `${dh}px`;

      setImageLoaded(true);
    };
    img.onerror = () => onCancel();
    img.src = imagePath;
    return () => {
      img.onload = null;
      img.onerror = null;
    };
  }, [imagePath, onCancel]);

  /* ── redraw overlay from refs ── */
  const redraw = useCallback(() => {
    const ov = overlayRef.current;
    if (!ov) return;
    const ctx = ov.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, ov.width, ov.height);
    for (const a of annotationsRef.current) drawAnnotation(ctx, a);
    if (drawingAnnRef.current) drawAnnotation(ctx, drawingAnnRef.current);
  }, []);

  useEffect(() => {
    redraw();
  }, [annotations, redraw]);

  /* ── coordinate helpers ── */
  const getCanvasPt = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>): Point => {
      const ov = overlayRef.current!;
      const r = ov.getBoundingClientRect();
      return {
        x: ((e.clientX - r.left) / r.width) * ov.width,
        y: ((e.clientY - r.top) / r.height) * ov.height,
      };
    },
    [],
  );

  /* ── mouse handlers ── */
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (e.button !== 0) return;

      if (tool === "text") {
        const pt = getCanvasPt(e);
        const r = overlayRef.current!.getBoundingClientRect();
        setTextInput({
          canvasPoint: pt,
          left: Math.min(e.clientX - r.left, r.width - 120),
          top: Math.min(e.clientY - r.top, r.height - 30),
        });
        return;
      }

      drawingRef.current = true;
      drawingAnnRef.current = { type: tool, color, points: [getCanvasPt(e)] };
    },
    [tool, color, getCanvasPt],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (!drawingRef.current || !drawingAnnRef.current) return;
      const pt = getCanvasPt(e);
      const cur = drawingAnnRef.current;

      drawingAnnRef.current =
        cur.type === "free"
          ? { ...cur, points: [...cur.points, pt] }
          : { ...cur, points: [cur.points[0], pt] };

      /* direct redraw (avoid React round-trip during mousemove) */
      const ov = overlayRef.current!;
      const ctx = ov.getContext("2d")!;
      ctx.clearRect(0, 0, ov.width, ov.height);
      for (const a of annotationsRef.current) drawAnnotation(ctx, a);
      drawAnnotation(ctx, drawingAnnRef.current);
    },
    [getCanvasPt],
  );

  const handleUpOrLeave = useCallback(() => {
    if (!drawingRef.current) return;
    drawingRef.current = false;
    const cur = drawingAnnRef.current;
    if (!cur) return;
    syncAnnotations([...annotationsRef.current, cur]);
    drawingAnnRef.current = null;
  }, [syncAnnotations]);

  /* ── text-input handlers ── */
  const commitText = useCallback(
    (txt: string) => {
      if (!textInput) return;
      syncAnnotations([
        ...annotationsRef.current,
        { type: "text", color, points: [textInput.canvasPoint], text: txt },
      ]);
      setTextInput(null);
    },
    [textInput, color, syncAnnotations],
  );

  const handleTextKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        const v = (e.target as HTMLInputElement).value.trim();
        if (v) commitText(v);
        else setTextInput(null);
      } else if (e.key === "Escape") {
        setTextInput(null);
      }
    },
    [commitText],
  );

  /* ── save (merge + write) ── */
  const handleSave = useCallback(async () => {
    const bg = bgCanvasRef.current!;
    const ov = overlayRef.current!;
    bg.getContext("2d")!.drawImage(ov, 0, 0);

    const blob = await new Promise<Blob>((r) => bg.toBlob((b) => r(b!), "image/png"));
    const path = `/tmp/feiq_annotated_${Date.now()}.png`;
    await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
    onSave(path);
  }, [onSave]);

  /* ── keyboard: Escape cancels overlay ── */
  useEffect(() => {
    const handler = (ev: globalThis.KeyboardEvent) => {
      if (ev.key === "Escape" && !textInput) onCancel();
    };
    globalThis.addEventListener("keydown", handler);
    return () => globalThis.removeEventListener("keydown", handler);
  }, [onCancel, textInput]);

  /* auto-focus text input when it appears */
  const textInputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    textInputRef.current?.focus();
  }, [textInput]);

  /* ── render ── */
  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-1 px-4 py-2 bg-gray-900 text-white shrink-0">
        <ToolBtn
          active={tool === "rect"}
          icon={<Square className="w-5 h-5" />}
          title="Rectangle"
          onClick={() => setTool("rect")}
        />
        <ToolBtn
          active={tool === "arrow"}
          icon={<ArrowRight className="w-5 h-5" />}
          title="Arrow"
          onClick={() => setTool("arrow")}
        />
        <ToolBtn
          active={tool === "text"}
          icon={<Type className="w-5 h-5" />}
          title="Text"
          onClick={() => setTool("text")}
        />
        <ToolBtn
          active={tool === "free"}
          icon={<Pen className="w-5 h-5" />}
          title="Free draw"
          onClick={() => setTool("free")}
        />

        <div className="w-px h-6 bg-white/20 mx-2" />

        {/* Color palette toggle */}
        <div className="relative">
          <ToolBtn
            active={false}
            icon={<Palette className="w-5 h-5" />}
            title="Color"
            onClick={() => setShowPalette((p) => !p)}
          />
          {showPalette && (
            <div className="absolute top-full left-0 mt-1 p-2 bg-gray-800 rounded-lg shadow-lg flex gap-1.5 z-10">
              {COLORS.map((c, i) => (
                <button
                  key={c}
                  onClick={() => {
                    setColor(c);
                    setShowPalette(false);
                  }}
                  className={`w-6 h-6 rounded-full border-2 transition-transform hover:scale-110 ${
                    color === c ? "border-white scale-110" : "border-transparent"
                  }`}
                  style={{ backgroundColor: c }}
                  title={COLOR_LABELS[i]}
                />
              ))}
            </div>
          )}
        </div>

        <ToolBtn
          active={false}
          icon={<Undo className="w-5 h-5" />}
          title="Undo"
          disabled={annotations.length === 0}
          onClick={() => {
            syncAnnotations(annotationsRef.current.slice(0, -1));
          }}
        />

        <div className="flex-1" />

        <ToolBtn
          active={false}
          icon={<X className="w-5 h-5" />}
          title="Cancel"
          onClick={onCancel}
        />
        <button
          onClick={handleSave}
          className="p-2 rounded-lg bg-green-600 hover:bg-green-500 transition-colors"
          title="Save & send"
        >
          <Check className="w-5 h-5" />
        </button>
      </div>

      {/* Canvas area */}
      <div className="flex-1 flex items-center justify-center overflow-hidden relative">
        {!imageLoaded ? (
          <div className="text-white/60 text-sm">Loading screenshot…</div>
        ) : (
          <div ref={containerRef} className="relative inline-block">
            <canvas ref={bgCanvasRef} className="block" />
            <canvas
              ref={overlayRef}
              className="absolute inset-0 block"
              style={{
                cursor: tool === "text" ? "text" : "crosshair",
              }}
              onMouseDown={handleMouseDown}
              onMouseMove={handleMouseMove}
              onMouseUp={handleUpOrLeave}
              onMouseLeave={handleUpOrLeave}
            />
            {textInput && (
              <input
                ref={textInputRef}
                type="text"
                defaultValue=""
                onKeyDown={handleTextKeyDown}
                onBlur={() => {
                  const el = textInputRef.current;
                  if (el && !el.value.trim()) setTextInput(null);
                }}
                className="absolute bg-transparent border-b-2 border-white text-white outline-none text-base min-w-[120px]"
                style={{
                  left: textInput.left,
                  top: textInput.top,
                }}
                autoFocus
              />
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/* ───────── small toolbar button wrapper ───────── */

function ToolBtn({
  active,
  icon,
  title,
  disabled,
  onClick,
}: {
  active: boolean;
  icon: React.ReactNode;
  title: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`p-2 rounded-lg transition-colors ${
        active ? "bg-white/20" : "hover:bg-white/10"
      } disabled:opacity-30 disabled:cursor-not-allowed`}
      title={title}
    >
      {icon}
    </button>
  );
}
