import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";

interface EmojiInfo {
  code: string;
  name: string;
  image: string;
}

interface Props {
  onSelect: (code: string) => void;
  onClose: () => void;
}

export function EmojiPicker({ onSelect, onClose }: Props) {
  const [emojis, setEmojis] = useState<EmojiInfo[]>([]);
  const [hovered, setHovered] = useState<number | null>(null);

  useEffect(() => {
    invoke<EmojiInfo[]>("get_emoji_list")
      .then(setEmojis)
      .catch(() => {
        // Fallback: generate emoji list from known codes
        const codes = [
          "/:)", "/:~", "/:*", "/:|", "/8-)", "/:<", "/:$", "/:X",
          "/:Z", "/:'(", "/:-|", "/:@", "/:P", "/:D", "/:O", "/<rotate>",
          "/:(", "/:+", "/:lenhan", "/:Q", "/:T", "/;P", "/;-D", "/;d",
          "/;o", "/:g", "/|-)", "/:!", "/:L", "/:>", "/;bin", "/:fw",
          "/;fd", "/:-S", "/;?", "/;x", "/;@", "/:8", "/;!", "/!!!",
          "/:xx", "/:bye", "/:csweat", "/:knose", "/:applause", "/:cdale",
          "/:huaixiao", "/:shake", "/:lhenhen", "/:rhenhen", "/:yawn",
          "/:snooty", "/:chagrin", "/:kcry", "/:yinxian", "/:qinqin",
          "/:xiaren", "/:kelin", "/:caidao", "/:xig", "/:bj",
          "/:basketball", "/:pingpong", "/:jump", "/:coffee", "/:eat",
          "/:pig", "/:rose", "/:fade", "/:kiss", "/:heart", "/:break",
          "/:cake", "/:shd", "/:bomb", "/:dao", "/:footb", "/:piaocon",
          "/:shit", "/:oh", "/:moon", "/:sun", "/;gift", "/:hug",
          "/:strong", "/;weak", "/:share", "/:shl", "/:baoquan",
          "/:cajole", "/:quantou", "/:chajin", "/:aini", "/:sayno",
          "/:sayok", "/:love",
        ];
        const names = [
          "微笑", "撇嘴", "色", "发呆", "得意", "流泪", "害羞", "闭嘴",
          "困", "大哭", "尴尬", "发怒", "调皮", "龇牙", "惊讶", "转圈",
          "难过", "酷", "冷汗", "抓狂", "吐", "偷笑", "可爱", "白眼",
          "傲慢", "饥饿", "困", "惊恐", "冷汗", "憨笑", "大兵", "飞吻",
          "奋斗", "咒骂", "疑问", "嘘", "晕", "折磨", "衰", "骷髅",
          "敲打", "再见", "擦汗", "抠鼻", "鼓掌", "糗", "坏笑", "发抖",
          "左哼哼", "右哼哼", "哈欠", "鄙视", "委屈", "快哭了", "阴险", "亲亲",
          "吓", "可怜", "菜刀", "西瓜", "啤酒", "篮球", "乒乓", "跳",
          "咖啡", "吃饭", "猪头", "玫瑰", "枯萎", "示爱", "爱心", "心碎",
          "蛋糕", "闪电", "炸弹", "匕首", "足球", "瓢虫", "大便", "怄火",
          "月亮", "太阳", "礼物", "拥抱", "点赞", "弱", "握手", "胜利",
          "抱拳", "勾引", "拳头", "差劲", "爱你", "no", "ok", "爱情",
        ];
        setEmojis(
          codes.map((code, i) => ({
            code,
            name: names[i] || "",
            image: `emojis/${i + 1}.gif`,
          }))
        );
      });
  }, []);

  return (
    <div className="absolute bottom-16 left-4 z-50 bg-white rounded-lg shadow-xl border border-gray-200 p-2 w-80">
      <div className="flex items-center justify-between mb-1 px-1">
        <span className="text-xs text-gray-400">
          {hovered !== null ? emojis[hovered]?.name : "Choose an emoji"}
        </span>
        <button onClick={onClose} className="p-0.5 hover:bg-gray-100 rounded cursor-pointer">
          <X className="w-3.5 h-3.5 text-gray-400" />
        </button>
      </div>
      <div
        className="grid gap-0.5"
        style={{ gridTemplateColumns: "repeat(16, 1fr)" }}
      >
        {emojis.map((emoji, i) => (
          <button
            key={emoji.code}
            onClick={() => onSelect(emoji.code)}
            onMouseEnter={() => setHovered(i)}
            onMouseLeave={() => setHovered(null)}
            className="w-4.5 h-4.5 flex items-center justify-center hover:bg-blue-50 rounded cursor-pointer
                       text-xs p-0 leading-none"
            title={emoji.name}
          >
            {/* Show emoji image with fallback */}
            <img
              src={emoji.image}
              alt={emoji.code}
              className="w-4 h-4 object-contain"
              loading="lazy"
              onError={(e) => {
                // Fallback: show emoji code text
                (e.target as HTMLImageElement).style.display = "none";
                const span = document.createElement("span");
                span.textContent = emoji.code.substring(0, 3);
                span.className = "text-[8px] text-gray-500";
                (e.target as HTMLImageElement).parentElement?.appendChild(span);
              }}
            />
          </button>
        ))}
      </div>
    </div>
  );
}
