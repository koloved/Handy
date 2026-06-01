import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getKeyName,
  formatKeyCombination,
  normalizeKey,
} from "@/lib/utils/keyboard";
import { useOsType } from "@/hooks/useOsType";

interface HotkeyPromptInputProps {
  value: string;
  onChange: (hotkey: string) => void;
  disabled?: boolean;
}

export const HotkeyPromptInput: React.FC<HotkeyPromptInputProps> = ({
  value,
  onChange,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const [isRecording, setIsRecording] = useState(false);
  const [pressedKeys, setPressedKeys] = useState<string[]>([]);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const osType = useOsType();

  useEffect(() => {
    if (!isRecording) return;

    let cleanup = false;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (cleanup) return;
      if (e.repeat) return;
      e.preventDefault();

      const rawKey = getKeyName(e, osType);
      const key = normalizeKey(rawKey);

      setPressedKeys((prev) => (prev.includes(key) ? prev : [...prev, key]));
      setRecordedKeys((prev) => (prev.includes(key) ? prev : [...prev, key]));
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (cleanup) return;
      e.preventDefault();

      const rawKey = getKeyName(e, osType);
      const key = normalizeKey(rawKey);

      setPressedKeys((prev) => prev.filter((k) => k !== key));

      const modifiers = [
        "ctrl", "control", "shift", "alt", "option",
        "meta", "command", "cmd", "super", "win", "windows",
      ];

      const updatedPressed = pressedKeys.filter((k) => k !== key);
      if (updatedPressed.length === 0 && recordedKeys.length > 0) {
        const sortedKeys = [...recordedKeys].sort((a, b) => {
          const aIsMod = modifiers.includes(a.toLowerCase());
          const bIsMod = modifiers.includes(b.toLowerCase());
          if (aIsMod && !bIsMod) return -1;
          if (!aIsMod && bIsMod) return 1;
          return 0;
        });
        const newHotkey = sortedKeys.join("+");
        onChange(newHotkey);
        setIsRecording(false);
        setPressedKeys([]);
        setRecordedKeys([]);
      }
    };

    const handleClickOutside = (e: MouseEvent) => {
      if (cleanup) return;
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsRecording(false);
        setPressedKeys([]);
        setRecordedKeys([]);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("click", handleClickOutside);

    return () => {
      cleanup = true;
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("click", handleClickOutside);
    };
  }, [isRecording, pressedKeys, recordedKeys, onChange, osType]);

  const formatCurrentKeys = (): string => {
    if (recordedKeys.length === 0) return t("settings.customPrompts.hotkey.pressKeys");
    return formatKeyCombination(recordedKeys.join("+"), osType);
  };

  return (
    <div ref={containerRef} className="flex items-center gap-2">
      {isRecording ? (
        <div className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded-md min-w-[120px]">
          {formatCurrentKeys()}
        </div>
      ) : (
        <div
          className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded-md min-w-[120px] cursor-pointer hover:border-logo-primary hover:bg-logo-primary/10 ${disabled ? "opacity-50 cursor-not-allowed" : ""}`}
          onClick={() => { if (!disabled) setIsRecording(true); }}
        >
          {value ? formatKeyCombination(value, osType) : t("settings.customPrompts.hotkey.clickToRecord")}
        </div>
      )}
      {value && (
        <button
          onClick={() => onChange("")}
          className="text-sm text-mid-gray hover:text-text ml-1"
          title={t("settings.customPrompts.hotkey.clear")}
        >
          {t("settings.customPrompts.hotkey.clearSymbol")}
        </button>
      )}
    </div>
  );
};
