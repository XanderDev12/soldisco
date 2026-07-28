"use client";

import { useEffect, useRef } from "react";

type StrategyUploadModalProps = {
  onClose: () => void;
};

export function StrategyUploadModal({
  onClose,
}: StrategyUploadModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    const previouslyFocused =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    closeButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const dialog = dialogRef.current;
      if (!dialog) return;
      const focusable = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
        ),
      ).filter((element) => !element.hasAttribute("hidden"));
      if (focusable.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (
        !event.shiftKey &&
        (active === last || !dialog.contains(active))
      ) {
        event.preventDefault();
        first.focus();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.requestAnimationFrame(() => previouslyFocused?.focus());
    };
  }, []);

  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        id="strategy-upload-dialog"
        className="modal"
        role="dialog"
        tabIndex={-1}
        aria-modal="true"
        aria-labelledby="upload-title"
        aria-describedby="upload-description"
      >
        <button
          ref={closeButtonRef}
          type="button"
          className="modal__close"
          onClick={onClose}
          aria-label="Close upload dialog"
        >
          ×
        </button>
        <span className="modal__eyebrow">STRATEGY WORKSPACE</span>
        <h2 id="upload-title">Upload a strategy</h2>
        <p id="upload-description">
          Strategy files will be accepted after validation and sandboxing are
          connected.
        </p>
        <div className="upload-zone">
          <span>⇧</span>
          <strong>No strategy file selected</strong>
          <small>Expected format: declarative JSON manifest</small>
        </div>
        <div className="modal__notice">
          Uploaded strategies will be inactive until validation and a replay
          dry run succeed.
        </div>
        <div className="modal__actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="button" disabled>Choose file</button>
        </div>
      </div>
    </div>
  );
}
