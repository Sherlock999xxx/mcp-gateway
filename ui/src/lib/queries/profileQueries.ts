import type { QueryClient } from "@tanstack/react-query";
import { qk } from "@/src/lib/queryKeys";

export async function invalidateProfile(queryClient: QueryClient, id: string) {
  await queryClient.invalidateQueries({ queryKey: qk.profile(id) });
}

export async function invalidateProfiles(queryClient: QueryClient) {
  await queryClient.invalidateQueries({ queryKey: qk.profiles() });
}
