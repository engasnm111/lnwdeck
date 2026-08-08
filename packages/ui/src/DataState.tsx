import type { ReactNode } from "react";
import { LoadingState } from "./LoadingState";
import { ErrorState } from "./ErrorState";
import { EmptyState } from "./EmptyState";

export interface DataStateProps {
  loading: boolean;
  error: Error | null;
  isEmpty: boolean;
  onRetry?: () => void;
  loadingFallback?: ReactNode;
  errorFallback?: ReactNode;
  emptyFallback?: ReactNode;
  labels?: DataStateLabels;
  children: ReactNode;
}

export interface DataStateLabels {
  loading: string;
  errorTitle: string;
  retry: string;
  emptyTitle: string;
  emptyDetail: string;
}

/**
 * Renders exactly one of loading, error, empty or content. An error is never
 * replaced by content, which is what allowed demo data to appear on failure.
 */
export function DataState({
  loading,
  error,
  isEmpty,
  onRetry,
  loadingFallback,
  errorFallback,
  emptyFallback,
  labels,
  children,
}: DataStateProps) {
  if (loading) {
    return <>{loadingFallback ?? <LoadingState label={labels?.loading} />}</>;
  }
  if (error) {
    return (
      <>
        {errorFallback ?? (
          <ErrorState
            error={error}
            onRetry={onRetry}
            title={labels?.errorTitle}
            retryLabel={labels?.retry}
          />
        )}
      </>
    );
  }
  if (isEmpty) {
    return (
      <>
        {emptyFallback ?? (
          <EmptyState
            title={labels?.emptyTitle}
            detail={labels?.emptyDetail}
          />
        )}
      </>
    );
  }
  return <>{children}</>;
}
