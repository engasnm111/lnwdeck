import { useCallback, useEffect, useLayoutEffect, useRef } from "react";

function sameDependencies(previous: readonly unknown[], next: readonly unknown[]) {
  return (
    previous.length === next.length &&
    previous.every((value, index) => Object.is(value, next[index]))
  );
}

/**
 * Invalidates async work as soon as its render-time request context changes.
 * Starting another request in the same context also makes the older one stale.
 */
export function useLatestRequestGuard(dependencies: readonly unknown[]) {
  const dependenciesRef = useRef(dependencies);
  const sequenceRef = useRef(0);
  const mountedRef = useRef(true);

  useLayoutEffect(() => {
    if (sameDependencies(dependenciesRef.current, dependencies)) return;
    dependenciesRef.current = dependencies;
    sequenceRef.current += 1;
  });

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      sequenceRef.current += 1;
    };
  }, []);

  return useCallback(() => {
    const sequence = ++sequenceRef.current;
    const requestDependencies = dependenciesRef.current;
    return () =>
      mountedRef.current &&
      sequenceRef.current === sequence &&
      dependenciesRef.current === requestDependencies;
  }, []);
}
