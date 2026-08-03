import { useCallback, useEffect, useState } from "react";
import { fetchOverview, OverviewData } from "../../lib/native";

export function TrayPopup() {
  const [data, setData] = useState<OverviewData | null>(null);

  const load = useCallback(async () => {
    try {
      const result = await fetchOverview();
      setData(result);
    } catch {
      // Silently handle in tray popup
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div style={{ padding: "0.5rem", fontSize: "0.875rem", mlnwidth: 200 }}>
      {data ? (
        <>
          <p>
            <strong>{data.total_events}</strong> events
          </p>
          <p>
            <strong>
              {(
                data.total_tokens_input + data.total_tokens_output
              ).toLocaleString()}
            </strong>{" "}
            tokens
          </p>
          <p>
            <strong>{data.provider_count}</strong> providers
          </p>
        </>
      ) : (
        <p>Loading...</p>
      )}
    </div>
  );
}
