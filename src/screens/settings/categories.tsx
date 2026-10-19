// Ustawienia › Archiwum › Kategorie plików: the archive's own file categories („Pocztówki”), with how many files use
// each, to rename or remove, and „+ Dodaj kategorię”. Renaming and removing change the files (an edit, so „Kto
// edytuje?” first); the list in the archive's settings follows once that has worked.

import { Plus } from "lucide-react";
import { useState } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { savedCategories, saveCategories, useCategories } from "../../app/categories";
import { afterChange, useStore } from "../../app/store";
import { SettingRow } from "../../components/bits";
import { CategoryDialog } from "../../components/CategoryDialog";
import { Dialog } from "../../components/Dialog";
import { BASE_LIKE, cleanName, type Base, type Category, validateName, withoutCategory, withRenamed } from "../../lib/categories";
import { count } from "../../lib/format";
import { runEdit } from "../media/shared";
import { Section } from "./parts";

/** What files of a removed category stay as: „jako zdjęcie” / „jako zdjęcia”. */
const STAYS_AS: Record<Base, [string, string]> = { photo: ["zdjęcie", "zdjęcia"], document: ["dokument", "dokumenty"], other: ["zwykły plik", "zwykłe pliki"] };

export function CategoriesSection({ archive }: { archive: ArchiveStatus }) {
  const categories = useCategories();
  const setAsk = useStore((s) => s.setAsk);
  const notify = useStore((s) => s.notify);
  const [adding, setAdding] = useState(false);
  const [renaming, setRenaming] = useState<Category | null>(null);
  if (archive.combined) return null;

  const remove = (c: Category) =>
    setAsk({
      title: `Usunąć kategorię „${c.name}”?`,
      text:
        c.count === 0
          ? "Żaden plik jej nie używa. Pliki nie zostaną usunięte."
          : `${count(c.count, "plik zostanie", "pliki zostaną", "plików zostanie")} jako ${STAYS_AS[c.base][c.count === 1 ? 0 : 1]}. Pliki nie zostaną usunięte.`,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              if (c.count > 0) await call("media.removeCategory", { name: c.name });
              await saveCategories(withoutCategory(savedCategories(), c.name));
              afterChange();
              notify(`Usunięto kategorię „${c.name}”.`);
            }),
        },
      ],
    });

  return (
    <Section id="categories" title="Kategorie plików">
      {categories.map((c) => (
        <SettingRow key={c.name} label={c.name} note={`${BASE_LIKE[c.base]} · ${c.count > 0 ? count(c.count, "plik", "pliki", "plików") : "bez plików"}`}>
          <button className="btn secondary set-btn" onClick={() => setRenaming(c)}>
            Zmień nazwę…
          </button>
          <button className="btn secondary set-btn" onClick={() => remove(c)}>
            Usuń…
          </button>
        </SettingRow>
      ))}
      <SettingRow
        label={categories.length === 0 ? "Własne kategorie" : "Nowa kategoria"}
        note={categories.length === 0 ? "Np. Pocztówki albo Listy. Pojawią się przy wyborze rodzaju pliku, obok Zdjęcia, Dokumentu i Inne." : undefined}
        last
      >
        <button className="btn secondary set-btn" onClick={() => setAdding(true)}>
          <Plus size={15} />
          Dodaj kategorię
        </button>
      </SettingRow>
      {adding && <CategoryDialog onClose={() => setAdding(false)} />}
      {renaming && <RenameDialog category={renaming} names={categories.map((c) => c.name)} onClose={() => setRenaming(null)} />}
    </Section>
  );
}

function RenameDialog({ category, names, onClose }: { category: Category; names: string[]; onClose: () => void }) {
  const notify = useStore((s) => s.notify);
  const [name, setName] = useState(category.name);
  const error = validateName(name, names, category.name);
  const unchanged = cleanName(name) === category.name;
  const shown = name.trim() && name !== category.name ? error : null;

  const rename = () => {
    if (error || unchanged) return;
    const to = cleanName(name);
    onClose();
    runEdit(async () => {
      if (category.count > 0) await call("media.renameCategory", { from: category.name, to });
      await saveCategories(withRenamed(savedCategories(), category.name, to));
      afterChange();
      notify(`Zmieniono nazwę kategorii na „${to}”.`);
    });
  };

  return (
    <Dialog width={440} onClose={onClose} labelledBy="category-rename-title">
      <div className="dialog-body">
        <div className="dialog-title" id="category-rename-title">
          Zmień nazwę kategorii
        </div>
        <label className="field">
          <span className="field-label">Nazwa</span>
          <input
            className="input"
            autoFocus
            value={name}
            maxLength={80}
            onFocus={(e) => e.target.select()}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && rename()}
            aria-invalid={shown ? true : undefined}
            aria-describedby={shown ? "category-rename-error" : undefined}
          />
          {shown && (
            <span id="category-rename-error" style={{ fontSize: 12, color: "var(--err)" }}>
              {shown}
            </span>
          )}
        </label>
        <div className="dialog-text">
          {category.count > 0 ? `Nowa nazwa pojawi się na ${count(category.count, "pliku", "plikach", "plikach")}.` : "Żaden plik jeszcze jej nie używa."}
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" onClick={rename} disabled={!!error || unchanged}>
          Zmień nazwę
        </button>
      </div>
    </Dialog>
  );
}
