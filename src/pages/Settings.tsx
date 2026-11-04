export const settingsPages = [
  "Applications",
  "Credentials",
  "Networks",
] as const;
export type Page = "Dashboard" | (typeof settingsPages)[number];
export const descriptions: Record<Page, string> = {
  Dashboard: "Wszystkie Twoje usługi. W jednym miejscu.",
  Applications: "Dodawaj usługi, ustawiaj adresy i wybierz sposób połączenia.",
  Credentials: "Dane logowania bezpiecznie zapisane na Twoim Macu.",
  Networks: "Sieci, które HomeLab zna i którym ufasz.",
};
