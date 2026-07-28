"use client";

import { useEffect, useState } from "react";

const compactNavigationQuery = "(max-width: 1020px)";

export function useCompactNavigation() {
  const [compact, setCompact] = useState(false);

  useEffect(() => {
    const media = window.matchMedia(compactNavigationQuery);
    const update = () => setCompact(media.matches);

    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  return compact;
}
