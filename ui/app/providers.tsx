"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
import { ToastViewport } from "@/components/ui";
import { isMissingTenantSessionError } from "@/src/lib/tenantFetch";

export function Providers({ children }: { children: ReactNode }) {
  const [client] = useState(() => {
    return new QueryClient({
      defaultOptions: {
        queries: {
          refetchOnWindowFocus: false,
          retry: (failureCount, error) => {
            if (isMissingTenantSessionError(error)) return false;
            return failureCount < 2;
          },
        },
        mutations: {
          retry: (failureCount, error) => {
            if (isMissingTenantSessionError(error)) return false;
            return failureCount < 1;
          },
        },
      },
    });
  });

  return (
    <QueryClientProvider client={client}>
      {children}
      <ToastViewport />
    </QueryClientProvider>
  );
}
