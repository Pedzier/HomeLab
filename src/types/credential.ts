export interface Credential {
  id: string;
  name: string;
  username?: string;
  secretReference: string;
  notes?: string;
}
