import { useEffect, useState } from "react";

export function usePersistentState<T>(
  storageKey: string,
  readInitialValue: () => T,
  serialize: (value: T) => string = String
) {
  const [value, setValue] = useState<T>(readInitialValue);

  useEffect(() => {
    window.localStorage.setItem(storageKey, serialize(value));
  }, [serialize, storageKey, value]);

  return [value, setValue] as const;
}
