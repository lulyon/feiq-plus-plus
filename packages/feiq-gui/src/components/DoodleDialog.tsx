import { useState, useRef, useCallback, useEffect } from "react";
import { Pencil, Eraser, Undo2, X, Send } from "lucide-react";

interface Props {
  peerIp: string;
  onClose: () => void;
}

type Tool = "pen" | "eraser";

const COLORS = [
  "#000000", "#FF0000", "#00AA00", "#0000FF", "#FF8800",
  "#8800FF", "#00AAAA", "#FF00FF", "#888888", "#FFD700",
];

export function DoodleDialog({ peerIp: _peerIp, onClose }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState("#000000");
  const [lineWidth, setLineWidth] = useState(3);
  const [drawing, setDrawing] = useState(false);
  const [sending, setSending] = useState(false);
  const [history, setHistory] = useState<ImageData[]>([]);
  const [historyIdx, setHistoryIdx] = useState(-1);

  // Initialize canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    saveState();
  }, []);

  const getCtx = () => canvasRef.current?.getContext("2d");

  const saveState = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
    setHistory((prev) => {
      const next = prev.slice(0, historyIdx + 1);
      next.push(data);
      return next.slice(-20); // keep last 20 states
    });
    setHistoryIdx((prev) => Math.min(prev + 1, 19));
  }, [historyIdx]);

  const undo = () => {
    if (historyIdx <= 0) return;
    const prevIdx = historyIdx - 1;
    const ctx = getCtx();
    if (!ctx) return;
    ctx.putImageData(history[prevIdx], 0, 0);
    setHistoryIdx(prevIdx);
  };

  const getPos = (e: React.MouseEvent | React.TouchEvent) => {
    const canvas = canvasRef.current!;
    const rect = canvas.getBoundingClientRect();
    let clientX: number, clientY: number;
    if ("touches" in e) {
      clientX = e.touches[0].clientX;
      clientY = e.touches[0].clientY;
    } else {
      clientX = e.clientX;
      clientY = e.clientY;
    }
    return {
      x: (clientX - rect.left) * (canvas.width / rect.width),
      y: (clientY - rect.top) * (canvas.height / rect.height),
    };
  };

  const startDraw = (e: React.MouseEvent | React.TouchEvent) => {
    const ctx = getCtx();
    if (!ctx) return;
    const pos = getPos(e);
    ctx.beginPath();
    ctx.moveTo(pos.x, pos.y);
    ctx.strokeStyle = tool === "eraser" ? "#ffffff" : color;
    ctx.lineWidth = tool === "eraser" ? lineWidth * 4 : lineWidth;
    setDrawing(true);
  };

  const draw = (e: React.MouseEvent | React.TouchEvent) => {
    if (!drawing) return;
    const ctx = getCtx();
    if (!ctx) return;
    const pos = getPos(e);
    ctx.lineTo(pos.x, pos.y);
    ctx.stroke();
  };

  const endDraw = () => {
    if (!drawing) return;
    const ctx = getCtx();
    if (ctx) ctx.closePath();
    setDrawing(false);
    saveState();
  };

  const handleSend = async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setSending(true);
    try {
      const blob = await new Promise<Blob | null>((resolve) =>
        canvas.toBlob(resolve, "image/png")
      );
      if (!blob) return;
      // Save to temp file and send via file transfer
      const arrayBuffer = await blob.arrayBuffer();
      const uint8 = new Uint8Array(arrayBuffer);
      // Write to a temp path using the Tauri filesystem
      // For now, convert to base64 and have the backend handle it
      const base64 = btoa(
        Array.from(uint8)
          .map((b) => String.fromCharCode(b))
          .join("")
      );
      // The doodle is sent as an IPMSG_SENDIMAGE — but since that's not
      // fully supported, we'll send it as a regular file.
      // Use invoke to save and send
      console.log("Doodle ready for send:", base64.length, "bytes");
    } catch (e) {
      console.error("Send doodle failed:", e);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-surface rounded-xl shadow-2xl border border-border flex flex-col">
        {/* Header / Toolbar */}
        <div className="px-3 py-2 border-b border-border flex items-center gap-2">
          <button
            onClick={() => setTool("pen")}
            className={`w-8 h-8 flex items-center justify-center rounded-md cursor-pointer
              ${tool === "pen" ? "bg-primary text-primary-foreground" : "hover:bg-surface-alt text-text-muted"}`}
            title="Pen"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <button
            onClick={() => setTool("eraser")}
            className={`w-8 h-8 flex items-center justify-center rounded-md cursor-pointer
              ${tool === "eraser" ? "bg-primary text-primary-foreground" : "hover:bg-surface-alt text-text-muted"}`}
            title="Eraser"
          >
            <Eraser className="w-4 h-4" />
          </button>
          <div className="w-px h-6 bg-border mx-1" />
          {COLORS.map((c) => (
            <button
              key={c}
              onClick={() => { setTool("pen"); setColor(c); }}
              className="w-6 h-6 rounded-full border-2 cursor-pointer flex-shrink-0"
              style={{
                backgroundColor: c,
                borderColor: color === c && tool === "pen" ? "var(--color-primary)" : "transparent",
                boxShadow: color === c && tool === "pen" ? "0 0 0 1px white, 0 0 0 3px var(--color-primary)" : undefined,
              }}
              title={c}
            />
          ))}
          <div className="w-px h-6 bg-border mx-1" />
          <input
            type="range"
            min="1"
            max="20"
            value={lineWidth}
            onChange={(e) => setLineWidth(Number(e.target.value))}
            className="w-20"
            title={`Line width: ${lineWidth}`}
          />
          <button
            onClick={undo}
            className="w-8 h-8 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer"
            title="Undo"
          >
            <Undo2 className="w-4 h-4" />
          </button>
          <div className="flex-1" />
          <button
            onClick={handleSend}
            disabled={sending}
            className="px-3 py-1 text-sm font-medium bg-primary text-primary-foreground
                       rounded-md hover:opacity-90 cursor-pointer disabled:opacity-50 flex items-center gap-1"
            title="Send as file"
          >
            <Send className="w-3.5 h-3.5" />
            Send
          </button>
          <button
            onClick={onClose}
            className="w-8 h-8 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer"
            title="Close"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Canvas */}
        <canvas
          ref={canvasRef}
          width={560}
          height={420}
          className="cursor-crosshair"
          onMouseDown={startDraw}
          onMouseMove={draw}
          onMouseUp={endDraw}
          onMouseLeave={endDraw}
          onTouchStart={startDraw}
          onTouchMove={draw}
          onTouchEnd={endDraw}
        />
      </div>
    </div>
  );
}
