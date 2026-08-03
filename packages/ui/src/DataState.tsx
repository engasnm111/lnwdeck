import type { ReactNode } from "react";
import { LoadingState } from "./LoadingState";
import { ErrorState } from "./ErrorState";
import { EmptyState } from "./EmptyState";

export interface DataStateProps {
  loading: boolean;
  error: Error | null;
  isEmpty: boolean;
  loadingFallback?: ReactNode;
  errorFallback?: ReactNode;
  emptyFallback?: ReactNode;
  children: ReactNode;
}

export function DataState({
  loading,
  error,
  isEmpty,
  loadingFallback,
  errorFallback,
  emptyFallback,
  children,
}: DataStateProps) {
  if (loading) {
    return <>{loadingFallback ?? <LoadingState />}</>;
  }
  if (error) {
    return <>{errorFallback ?? <ErrorState error={error} />}</>;
  }
  if (isEmpty) {
    return <>{emptyFallback ?? <EmptyState />}</>;
  }
  return <>{children}</>;
}
