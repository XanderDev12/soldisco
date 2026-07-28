"use client";

import { useRef, type KeyboardEvent } from "react";
import type { ExecutionMode } from "./types";

const executionModes = ["Paper", "Live"] as const;

type ExecutionModeSwitchProps = {
  mode: ExecutionMode;
  onModeChange: (mode: ExecutionMode) => void;
};

export function ExecutionModeSwitch({
  mode,
  onModeChange,
}: ExecutionModeSwitchProps) {
  const modeButtons =
    useRef<Partial<Record<ExecutionMode, HTMLButtonElement>>>({});

  function selectAndFocus(nextMode: ExecutionMode) {
    onModeChange(nextMode);
    window.requestAnimationFrame(() => {
      modeButtons.current[nextMode]?.focus();
    });
  }

  function handleKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    let nextIndex: number | null = null;

    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (index + 1) % executionModes.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex =
        (index - 1 + executionModes.length) % executionModes.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = executionModes.length - 1;
    }

    if (nextIndex === null) return;
    event.preventDefault();
    selectAndFocus(executionModes[nextIndex]);
  }

  return (
    <div
      className="mode-switch"
      role="radiogroup"
      aria-label="Execution mode"
    >
      {executionModes.map((item, index) => (
        <button
          ref={(element) => {
            if (element) modeButtons.current[item] = element;
          }}
          type="button"
          role="radio"
          aria-checked={mode === item}
          tabIndex={mode === item ? 0 : -1}
          key={item}
          className={mode === item ? "is-active" : ""}
          onClick={() => onModeChange(item)}
          onKeyDown={(event) => handleKeyDown(event, index)}
        >
          {item}
        </button>
      ))}
    </div>
  );
}
