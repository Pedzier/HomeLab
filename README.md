# HomeLab

Lokalny launcher usług homelabu dla **macOS 14+ / Apple Silicon**. Tauri 2, React, TypeScript, Vite i Tailwind CSS. Usługi domyślnie otwierają się w głównym oknie HomeLab jako natywny WKWebView, z górnym paskiem i przyciskiem powrotu, bez iframe. Ikony pozwalają wybrać osobne okno HomeLab lub przeglądarkę systemową.

## Uruchomienie

Wymagane: Node.js 22.12+ (lub 24+), Rust stable oraz Xcode Command Line Tools.

```sh
npm ci
npm run tauri dev
```

Podczas przygotowania PoC Rust został zainstalowany w `/private/tmp/homelab-cargo` i `/private/tmp/homelab-rustup`. Skrypt `scripts/native.mjs` używa tej instalacji, jeśli `cargo` nie jest dostępne w PATH. Katalog tymczasowy może zostać usunięty przez system; do stałej pracy zainstaluj Rust z https://rustup.rs.

```sh
npm run build                      # TypeScript + frontend produkcyjny
npm run lint                       # Oxlint
npm test                           # testy Rust: resolver, TCP, storage, URL
npm run test:keychain               # rzeczywisty test jednorazowego wpisu Keychain
npm run tauri build -- --bundles app # kompilacja pakietu macOS
```

Szybszy pakiet PoC: `npm run tauri build -- --debug --bundles app`.
Wynik: `src-tauri/target/debug/bundle/macos/HomeLab.app` (lub `release` dla zwykłego builda). Pakiet lokalny nie jest podpisany certyfikatem Developer ID ani notaryzowany.

`npm run dev` udostępnia wyłącznie podgląd frontendowy. WebView, sieci, konfiguracja i Keychain wymagają uruchomienia Tauri.

## Pierwsze użycie

1. Dashboard ma cztery przykładowe kafelki: Home Assistant, Proxmox, n8n i AI Services. Home Assistant używa podanego `http://192.168.0.20:8123`, AI Services `http://192.168.0.51`. Pozostałe adresy są przykładami.
2. W **Settings → Applications** ustaw rzeczywiste Local URL i Remote / Tailscale URL. Możesz dodawać, edytować, usuwać i przestawiać aplikacje strzałkami.
3. W **Settings → Networks** sprawdź wykrytą sieć. Przy tylko jednej dostępnej usłudze oznacz sieć ręcznie przez **Trust network**, jeśli to Twoja domowa sieć. Dwa różne odpowiadające endpointy w bieżącej podsieci LAN uczą ją automatycznie.
4. Kliknij kafelek, aby otworzyć usługę w głównym oknie pod górnym paskiem. Przycisk HomeLab wraca do dashboardu. Ikona okna na kafelku lub pasku otwiera osobne okno HomeLab, a ikona przeglądarki — przeglądarkę systemową. Status Offline nie blokuje ręcznej próby otwarcia istniejącego adresu.
5. Logujesz się bezpośrednio w oknie usługi. Ekran Credentials i jego wybór w formularzu aplikacji są tymczasowo ukryte przełącznikiem `credentialsEnabled` w `src/features.ts`. Istniejące przypisania i wpisy Keychain pozostają zachowane; ukrycie funkcji nie usuwa danych.

## Wybór połączenia

- **Auto / Trusted LAN:** wybiera Local URL bez testu endpointu przed otwarciem.
- **Auto / sieć z ręcznie wymuszonym Tailscale:** wybiera Remote URL.
- **Auto / nieznana sieć:** testuje do sześciu różnych lokalnych par host:port równolegle. Cały test pojedynczego endpointu, łącznie z DNS, ma limit 400 ms. Liczą się tylko prywatne adresy w bieżącej podsieci, osiągane przez ten sam interfejs fizyczny; publiczne strony i trasy przez VPN nie potwierdzają LAN. Minimum dwa sukcesy oznaczają dostęp LAN. Z dostępnym MAC bramy decyzję można wykorzystać ponownie bez testowania usług. Po nieudanym automatycznym rozpoznaniu używa Remote URL i ponawia uczenie po wygaśnięciu 30-sekundowej pamięci decyzji. Ręcznie wybrany Tailscale nie jest nadpisywany.
- **Local / Remote:** wymusza wybrany adres niezależnie od zaufania do sieci.
- Brak Remote URL poza zaufaną siecią daje Offline i komunikat wyjaśniający brak konfiguracji.

Fingerprint obejmuje interfejs, jego typ, IP i MAC domyślnej bramy, SSID jeśli dostępne i właściwy subnet (bez adresu hosta przydzielonego przez DHCP). Subnet sam nie wystarcza do zaufania. SSID może być ukryty przez macOS — HomeLab nie wymaga uprawnienia lokalizacji. Jeśli macOS nie udostępnia MAC bramy, HomeLab nadal wybiera LAN po dwóch udanych testach usług, ale po wygaśnięciu 30-sekundowej pamięci i po restarcie weryfikuje dostęp ponownie. Sam zapis profilu bez MAC nie wystarcza do wyboru LAN. Ręczne „Zaufaj tej sieci” działa także bez MAC: zapisuje wybór dla dostępnego zestawu danych (interfejs, brama, podsieć, typ i SSID, jeśli dostępny). Ręczny wybór LAN lub Tailscale ma pierwszeństwo przed automatycznym uczeniem.

Resolver przechowuje decyzję o bieżącej sieci w pamięci przez 30 sekund. W Auto przed ponownym użyciem porównuje bieżący interfejs, bramę, jej MAC i subnet; pełny odczyt metadanych Wi-Fi nie blokuje każdego kliknięcia. Zmiana tej tożsamości wymaga ponownego rozpoznania, a ręczne Trust/Forget unieważnia pamięć decyzji. Także po restarcie znany profil może zostać dopasowany po interfejsie, bramie, MAC i subnecie. Tryby Local i Remote całkowicie pomijają wykrywanie sieci przy otwieraniu.

Statusy odświeżają się w tle co 30 sekund i przyciskiem Odśwież. Także odświeżenie otwartej usługi ponownie wybiera trasę i aktualizuje adres na pasku. Odświeżenie unieważnia pamięć decyzji, więc może od razu ponowić sprawdzenie LAN po zmianie uprawnienia macOS. Jest to test dostępności TCP, a nie poprawności HTTP, logowania czy certyfikatu. Przekroczenie limitu, błąd DNS lub brak uprawnień do testu nie oznaczają Offline: kafelek zachowuje etykietę LAN/Tailscale z neutralnym kolorem i wyjaśnieniem po najechaniu. Offline oznacza brak adresu albo jawną odmowę połączenia. Załadowanie strony w WebView potwierdza dostępność dla danej aplikacji i originu przez 90 sekund; adres LAN nie potwierdza dostępności adresu Tailscale. Licznik online uwzględnia tylko potwierdzone usługi. Tryb Auto nie przełącza się na Remote tylko dlatego, że pojedyncza usługa w zaufanej sieci przestała odpowiadać.

**Forget network** usuwa zapamiętaną decyzję. Przy następnym odświeżeniu aktualna sieć zostanie ponownie wykryta i nauczona. Po dodaniu endpointów nieudane automatyczne rozpoznanie ponowi się także bez usuwania profilu. Starsze niezaufane profile bez zapisanej ręcznej decyzji są ponownie sprawdzane.

Jeżeli adresy LAN działają w przeglądarce, a resolver wybiera Tailscale, sprawdź **Ustawienia systemowe → Prywatność i ochrona → Sieć lokalna**. Przy uruchamianiu z terminala VS Code wymagany może być dostęp dla **Visual Studio Code**; spakowana aplikacja potrzebuje dostępu dla **HomeLab**. Po przyznaniu dostępu uruchom aplikację ponownie. Pierwsza odmowa lub brak odpowiedzi nie jest już zapamiętywany jako trwały wybór Tailscale.

Tailscale musi być samodzielnie zainstalowany i połączony; HomeLab nie zarządza VPN. Domyślna trasa przez VPN może tworzyć inny profil sieci. Dla niezaufanego certyfikatu HTTPS HomeLab wyświetla własne natywne okno potwierdzenia. Można anulować, połączyć się tym razem lub zapamiętać konkretny certyfikat dla danej aplikacji, hosta i portu. Nie zmienia to zaufania w systemie macOS.

## Dane i bezpieczeństwo

- Konfiguracja: `~/Library/Application Support/pl.homelab.desktop/settings.json`, zapisywana atomowo z uprawnieniami `0600`. Zawiera aplikacje, profile sieci oraz metadane credentiali.
- Hasła: wyłącznie macOS Keychain, service `pl.homelab.desktop`, account równy UUID credentiala. Hasło nie jest zwracane do JavaScript podczas kopiowania; Rust przekazuje je bezpośrednio do `pbcopy` przez stdin. Nowo wpisane hasło istnieje chwilowo w pamięci formularza i IPC, a nie w localStorage czy pliku ustawień.
- Schowek pozostaje pod kontrolą systemu; po skopiowaniu nadpisz hasło inną zawartością, gdy skończysz go używać. Nie wpisuj sekretów w nazwach, notatkach ani URL; URL z loginem lub hasłem jest odrzucany.
- Każda usługa ma trwały WKWebsiteDataStore identyfikowany swoim UUID (macOS 14+); sesje i cookies mogą pozostać po zamknięciu okna i restarcie. LAN i Tailscale mają różne originy i mogą wymagać osobnych logowań.
- Tylko lokalny webview `main` ma capability Tauri (przypisane do webview, a nie całego okna). Osadzony webview usługi i osobne okna usług nie mają uprawnień natywnych; komendy dodatkowo weryfikują etykietę wywołującego webview.
- Błędny plik ustawień powoduje błąd uruchomienia zamiast cichego nadpisania konfiguracji.

## Układ projektu

`src/components` i `src/pages` zawierają interfejs. `src/services` jest cienką warstwą wywołań natywnych, a `src/types` modelami danych. Osadzanie natywnego WebView korzysta z funkcji `unstable` Tauri 2 (API multiwebview). Główne okno jest tworzone jako okno z osobnymi natywnymi WebView. Pasek ma stałe 64 logiczne piksele; backend odczytuje bezpieczny obszar treści z AppKit na głównym wątku i dopasowuje obszar usługi przy zmianie rozmiaru okna i skali ekranu. Nie wyznacza wysokości paska tytułu przez różnicę rozmiarów Tauri. Powrót ukrywa osadzony widok, zachowując jego aktualną stronę i stan w pamięci. Trzy ostatnio używane widoki pozostają gotowe do ponownego pokazania; starsze są zamykane. Ponowne kliknięcie nie przeładowuje strony, jeśli resolver wybrał ten sam URL. Zmiana trasy LAN/Tailscale albo adresu w konfiguracji wymusza nawigację do nowego adresu. Przycisk „Odśwież usługę” na pasku pozwala ręcznie przeładować bieżącą stronę. Cookies pozostają w tym samym magazynie co osobne okno usługi.

Resolver, wykrywanie sieci i testy TCP są w `src-tauri/src/commands/network.rs`, okna w `webview.rs`, Keychain w `credentials.rs`. `store.rs` obsługuje walidację i trwały zapis.

## Weryfikacja

Automatyczne testy sprawdzają tryby Auto/Local/Remote, brak domyślnego dostępu LAN w nieznanej sieci, stabilność subnetu przy zmianie DHCP, połączenie z rzeczywistym portem localhost, odrzucanie niebezpiecznych URL i atomowy zapis ustawień. `test:keychain` wykonuje rzeczywisty cykl create/read/update/delete na jednorazowym wpisie.

Do sprawdzenia we własnej infrastrukturze: zaufaj domowej sieci, otwórz Home Assistant, zaloguj się i uruchom aplikację ponownie; potem przejdź do innej sieci z włączonym Tailscale i potwierdź wybór adresu zdalnego. Testy localhost nie zastępują tego sprawdzenia dwóch rzeczywistych sieci.

Dokumentacja API: [Tauri WebviewWindowBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html), [capabilities Tauri 2](https://v2.tauri.app/security/capabilities/), [Tailwind z Vite](https://tailwindcss.com/docs/installation/using-vite).

## Proxmox: działa w przeglądarce, ale nie w HomeLab

Wyjątek certyfikatu zaakceptowany w przeglądarce nie jest automatycznie wyjątkiem dla WKWebView HomeLab. Proxmox domyślnie używa certyfikatu podpisanego przez prywatne CA klastra. Dodatkowo certyfikat musi obejmować nazwę lub IP użyte w URL (Subject Alternative Name).

Sprawdzenie połączenia `https://100.107.29.27:8006` wykazało niezaufanego wystawcę. Odczytany certyfikat ma nazwę `pve.pve.local` i obejmuje m.in. `pve`, `pve.pve.local` i `192.168.0.49`; nie obejmuje IP Tailscale `100.107.29.27`. Samo zaufanie do CA nie naprawia niezgodności adresu.

HomeLab obsługuje teraz ten przypadek własnym oknem macOS. Po kliknięciu usługi z niezaufanym certyfikatem pojawia się pytanie z adresem, informacjami o certyfikacie, powodem odrzucenia i odciskiem SHA-256:

- **Anuluj** — przerywa połączenie, domyślna akcja.
- **Połącz tym razem** — pozwala na ten konkretny certyfikat w bieżącym widoku, maksymalnie do 8 godzin (do godziny w przypadku nieprawidłowej daty certyfikatu). Nie zapisuje wyjątku na dysku.
- **Zaufaj i zapamiętaj** — zapisuje wyjątek w ustawieniach HomeLab, maksymalnie na 30 dni i nie dłużej niż ważność certyfikatu. Opcja jest dostępna tylko dla certyfikatów ważnych czasowo.

Wyjątek jest dopasowany do UUID aplikacji, hosta, portu i SHA-256 całego certyfikatu serwera. Inny niezaufany certyfikat wymaga nowej zgody. Przekierowanie na inny host nie dziedziczy wyjątku. Dla prawidłowego systemowo zaufanego certyfikatu pozostaje standardowa obsługa TLS. Okno wyjątków jest dostępne wyłącznie dla błędów oznaczonych przez Security.framework jako możliwe do zaakceptowania przez użytkownika; jawne odrzucenie systemowe i błędy krytyczne nie są omijane nawet przez zapisany wyjątek. W **Applications → Edytuj → Usuń zaufanie do certyfikatów** można usunąć wyjątki danej aplikacji; zamyka to jej istniejące widoki i usuwa także zgodę tymczasową.

Okno jest natywnym `NSAlert` nad właściwym oknem usługi, obsługiwanym przez `WKNavigationDelegate`. Decyzja użytkownika pozostaje po stronie macOS/Rust; strona usługi nie otrzymuje uprawnień do zatwierdzania certyfikatów. Własny delegate przekazuje pozostałe callbacki do oryginalnego delegate Wry. To obsługa platformowa dla macOS, nie globalne wyłączenie weryfikacji HTTPS.

Alternatywy administracyjne, jeśli chcesz uniknąć wyjątków w aplikacji:

- Użyj zaufanego certyfikatu serwera dla wybranej nazwy DNS, skieruj tę nazwę na serwer osiągalny przez Tailscale i ustaw w HomeLab URL z tą nazwą.
- Przy pozostaniu przy prywatnym CA: pobierz `/etc/pve/pve-root-ca.pem` przez istniejące, zaufane połączenie SSH, zweryfikuj jego pochodzenie i samodzielnie dodaj zaufanie SSL w macOS Keychain. Używaj nazwy objętej certyfikatem, np. `https://pve.pve.local:8006`, z prawidłowym rozwiązywaniem tej nazwy na IP Tailscale. Zaufanie do CA obejmuje certyfikaty, które ono podpisuje; nie importuj jako zaufanego CA pliku nieznanego pochodzenia.

HomeLab nie zmienia zaufania certyfikatów w macOS; zaakceptowane wyjątki dotyczą jedynie konkretnego połączenia w HomeLab. Nie zastępuj plików CA ani automatycznie generowanych certyfikatów w `/etc/pve` przypadkowymi plikami. Zmiana certyfikatu proxy Proxmoxa powinna używać jego ekranu Node → Certificates.

Źródła: [Proxmox Certificate Management](https://github.com/proxmox/pve-docs/blob/master/certificate-management.adoc), [ustawienia zaufania certyfikatów w macOS](https://support.apple.com/en-ie/guide/keychain-access/kyca11871/mac).
