"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { SoldiscoApiClient } from "../../../lib/soldisco-api/client";
import { resolveBrowserApiUrl } from "../../../lib/soldisco-api/config";
import type {
  QualificationDefaultsResponse,
  UpdateQualificationDefaultsRequest,
} from "../../../lib/soldisco-api/contracts";
import { normalizeApiError } from "../../../lib/soldisco-api/errors";

type SettingsRequestStatus =
  | "IDLE"
  | "LOADING"
  | "READY"
  | "SAVING"
  | "ERROR";

export function useQualificationDefaults(enabled: boolean) {
  const mountedRef = useRef(false);
  const clientRef = useRef<SoldiscoApiClient | null>(null);
  const [settings, setSettings] =
    useState<QualificationDefaultsResponse | null>(null);
  const [status, setStatus] = useState<SettingsRequestStatus>("IDLE");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const client = clientRef.current;
    if (!client) return;
    setStatus("LOADING");
    setErrorMessage(null);
    try {
      const response = await client.qualificationDefaults;
      if (!mountedRef.current || clientRef.current !== client) return;
      setSettings(response);
      setStatus("READY");
    } catch (reason) {
      if (!mountedRef.current || clientRef.current !== client) return;
      setErrorMessage(normalizeApiError(reason).message);
      setStatus("ERROR");
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    if (!enabled) {
      clientRef.current = null;
      return () => {
        mountedRef.current = false;
      };
    }

    const resolution = resolveBrowserApiUrl();
    if (resolution.baseUrl === null) {
      queueMicrotask(() => {
        if (!mountedRef.current) return;
        setErrorMessage("The local settings API is unavailable.");
        setStatus("ERROR");
      });
      return () => {
        mountedRef.current = false;
      };
    }

    const client = new SoldiscoApiClient(resolution.baseUrl);
    clientRef.current = client;
    void refresh();

    return () => {
      mountedRef.current = false;
      if (clientRef.current === client) clientRef.current = null;
    };
  }, [enabled, refresh]);

  const save = useCallback(
    async (
      request: UpdateQualificationDefaultsRequest,
    ): Promise<QualificationDefaultsResponse | null> => {
      const client = clientRef.current;
      if (!client) return null;
      setStatus("SAVING");
      setErrorMessage(null);
      try {
        const response =
          await client.updateQualificationDefaults(request);
        if (!mountedRef.current || clientRef.current !== client) return null;
        setSettings(response);
        setStatus("READY");
        return response;
      } catch (reason) {
        if (!mountedRef.current || clientRef.current !== client) return null;
        setErrorMessage(normalizeApiError(reason).message);
        setStatus("ERROR");
        return null;
      }
    },
    [],
  );

  return { settings, status, errorMessage, refresh, save };
}
