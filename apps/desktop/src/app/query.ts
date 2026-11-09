import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Number.POSITIVE_INFINITY,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
    },
    mutations: {
      retry: false,
    },
  },
});

export const queryKeys = {
  entries: ["entries"],
  entry: (name: string) => ["entry", name],
  entryAll: ["entry"],
  devices: ["devices"],
  totp: (name: string) => ["totp", name],
} as const;
