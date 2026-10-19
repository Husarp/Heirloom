// „Nowa kategoria”: the name of an own file category and the kind it behaves like. Used wherever a file's kind is
// chosen (import steps 1 and 4, Media) and in Ustawienia.

import { useState } from "react";
import { savedCategories, saveCategories, useCategories } from "../app/categories";
import { BASE_LABEL, BASES, type Base, cleanName, validateName, withAdded } from "../lib/categories";
import { Segmented } from "./bits";
import { Dialog } from "./Dialog";

/** Adds the category to the archive's list when „Dodaj” is pressed, then tells the caller (`onAdded`), which usually
 *  gives the file in question that category. `base`: what „Traktuj jak” starts as (the file's current kind). */
export function CategoryDialog({ base = "photo", onClose, onAdded }: { base?: Base; onClose: () => void; onAdded?: (name: string, base: Base) => void }) {
  const categories = useCategories();
  const [name, setName] = useState("");
  const [like, setLike] = useState<Base>(base);
  const [busy, setBusy] = useState(false);
  const error = validateName(name, categories.map((c) => c.name));
  // Not shown for an empty field: „Dodaj” is only grey until something is typed.
  const shown = name.trim() ? error : null;

  const add = async () => {
    if (error || busy) return;
    const clean = cleanName(name);
    setBusy(true);
    const saved = await saveCategories(withAdded(savedCategories(), clean, like));
    setBusy(false);
    if (!saved) return;
    onClose();
    onAdded?.(clean, like);
  };

  return (
    <Dialog width={440} onClose={onClose} labelledBy="category-title">
      <div className="dialog-body">
        <div className="dialog-title" id="category-title">
          Nowa kategoria
        </div>
        <label className="field">
          <span className="field-label">Nazwa</span>
          <input
            className="input"
            autoFocus
            value={name}
            maxLength={80}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
            placeholder="Np. Pocztówki"
            aria-invalid={shown ? true : undefined}
            aria-describedby={shown ? "category-error" : undefined}
          />
          {shown && (
            <span id="category-error" style={{ fontSize: 12, color: "var(--err)" }}>
              {shown}
            </span>
          )}
        </label>
        <div className="field">
          <span className="field-label">Traktuj jak</span>
          <Segmented variant="neutral" full value={like} onChange={setLike} options={BASES.map((b) => ({ value: b, label: BASE_LABEL[b] }))} />
          <span style={{ fontSize: 12, color: "var(--text3)" }}>
            {like === "photo" && "Pliki z tej kategorii pokażą się w Galerii i mogą być zdjęciem profilowym."}
            {like === "document" && "Pliki z tej kategorii pokażą się w Dokumentach osoby."}
            {like === "other" && "Zwykłe pliki — nie trafią do Galerii."}
          </span>
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" onClick={add} disabled={!!error || busy}>
          Dodaj
        </button>
      </div>
    </Dialog>
  );
}
