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
  children: ReactNode;
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
  children,
}: DataStateProps) {
  if (loading) {
    return <>{loadingFallback ?? <LoadingState />}</>;
  }
  if (error) {
    return (
      <>{errorFallback ?? <ErrorState error={error} onRetry={onRetry} />}</>
    );
  }
  if (isEmpty) {
    return <>{emptyFallback ?? <EmptyState />}</>;
  }
  return <>{children}</>;
}
