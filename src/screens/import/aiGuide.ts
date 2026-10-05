// Import · 1 Wczytaj: the step-by-step guide „Jak przygotować paczkę z pomocą AI”, in the words of the real buttons and
// of docs/AI_INSTRUCTIONS.md (aiGuide.test.ts checks both).

export interface GuideStep {
  /** The bold start of the step. */
  title: string;
  text: string;
  /** „Kopiuj instrukcję dla AI” sits in this step. */
  copy?: boolean;
}

export const GUIDE_STEPS: GuideStep[] = [
  {
    title: "Zbierz materiał w jednym folderze:",
    text: "zdjęcia, skany aktów, PDF-y, notatki. Nazwę każdego pliku zacznij od numeru i spacji: „M001 akt urodzenia Józefa.jpg”, „M002 …”.",
  },
  {
    title: "Skopiuj instrukcję dla AI",
    text: "— jest w niej ten opis krok po kroku, a dla czatu Część B.",
    copy: true,
  },
  {
    title: "Otwórz nową rozmowę z AI",
    text: "(ChatGPT, Claude, Gemini) i wklej instrukcję jako pierwszą wiadomość. Potem dołącz pliki i napisz, które numery wgrywasz, np. „Wgrywam M001–M006”. Tekst notatek możesz wkleić wprost.",
  },
  {
    title: "Sprawdź przepisane akty",
    text: "— AI najpierw je przepisze. Popraw imiona, nazwiska, daty i miejscowości, a potem napisz: ZRÓB PLIK. Gdy AI poprosi, napisz „dalej” — dostaniesz kolejną część.",
  },
  {
    title: "Skopiuj odpowiedź",
    text: "przyciskiem „Kopiuj” przy każdym bloku kodu (albo zapisz ją w folderze jako czesc-1.json, czesc-2.json…).",
  },
  {
    title: "Wróć tutaj",
    text: "i wklej odpowiedź w pole „Wklej odpowiedź AI” albo upuść cały folder — odpowiedzi razem z plikami M… — na „Upuść wszystko naraz”.",
  },
  {
    title: "Heirloom sprawdzi odpowiedź",
    text: "i poprowadzi przez kroki Sprawdź, Dopasuj osoby, Zdjęcia i pliki i Podsumowanie. Nic się nie zapisze, dopóki nie zatwierdzisz podsumowania.",
  },
];

/** Opened at first, while the archive has had no import yet; closed once the imports' history is known to be there. */
export function guideOpenByDefault(history: unknown[] | null | undefined): boolean {
  return !!history && history.length === 0;
}
