import { ArrowRight, Files, UserPlus } from "lucide-react";
import { useStore } from "../../app/store";
import { BrandMark } from "../../components/bits";

/** Nowe, puste archiwum (spec §4.22, design 17c): the files the family already has come first, adding by hand second. */
export function EmptyArchive() {
  const go = useStore((s) => s.go);
  const requireEdit = useStore((s) => s.requireEdit);
  const closeArchive = useStore((s) => s.closeArchive);
  const whenSaved = useStore((s) => s.whenSaved);
  return (
    <div className="page empty-start">
      <div className="col" style={{ maxWidth: 760, gap: 24, alignItems: "center", textAlign: "center" }}>
        <BrandMark size={72} />
        <div className="col" style={{ gap: 10 }}>
          <h1 className="serif" style={{ fontSize: 44, lineHeight: 1.1, fontWeight: 500 }}>
            Zacznij archiwum rodziny
          </h1>
          <p style={{ fontSize: 17, lineHeight: 1.6, color: "var(--text2)" }}>
            Wszystko zostaje na tym komputerze. Najszybciej zaczniesz od plików, które już masz: odpowiedzi AI, zdjęć i skanów. Możesz też
            dodać pierwszą osobę ręcznie.
          </p>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, width: "100%", textAlign: "left" }}>
          <div className="card col" style={{ padding: 22, gap: 10, border: "1.5px solid var(--accent)" }}>
            <Files size={22} color="var(--accent-text)" />
            <span className="serif" style={{ fontSize: 21, fontWeight: 600 }}>
              Wczytaj pliki rodziny
            </span>
            <span style={{ fontSize: 14, lineHeight: 1.5, color: "var(--text2)" }}>
              Upuść naraz odpowiedzi AI, zdjęcia, skany i notatki. Heirloom rozpozna każdy plik, a wszystko sprawdzisz przed zapisem.
            </span>
            <button className="btn primary lg" style={{ marginTop: 6, alignSelf: "flex-start" }} onClick={() => go({ name: "import" })}>
              Otwórz Import
              <ArrowRight size={15} />
            </button>
          </div>
          <div className="card col" style={{ padding: 22, gap: 10 }}>
            <UserPlus size={22} color="var(--accent-text)" />
            <span className="serif" style={{ fontSize: 21, fontWeight: 600 }}>
              Dodaj pierwszą osobę
            </span>
            <span style={{ fontSize: 14, lineHeight: 1.5, color: "var(--text2)" }}>
              Zwykle najstarszy znany przodek albo ktoś, od kogo chcesz zacząć drzewo.
            </span>
            <button className="btn secondary lg" style={{ marginTop: 6, alignSelf: "flex-start" }} onClick={() => requireEdit(() => go({ name: "edit", id: null }))}>
              Dodaj osobę
            </button>
          </div>
        </div>
        <p style={{ fontSize: 14, color: "var(--text2)" }}>
          Masz już archiwum lub kopię zapasową?{" "}
          {/* Empty can still mean unsaved changes (the only person's adding undone, say). */}
          <button className="link" onClick={() => whenSaved(closeArchive, "Zmieniasz archiwum")}>
            Otwórz istniejące
          </button>
        </p>
      </div>
    </div>
  );
}
