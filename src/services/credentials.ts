import { native } from "./native";
export const saveCredential = (
  id: string,
  name: string,
  username: string,
  notes: string,
  password: string | null,
) => native<void>("save_credential", { id, name, username, notes, password });
export const deleteCredential = (id: string) =>
  native<void>("delete_credential", { id });
export const copyCredential = (id: string, field: "username" | "password") =>
  native<void>("copy_credential", { id, field });
