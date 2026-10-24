// „Do uzupełnienia” on a profile: the note at the top of the article and the „⋯” menu in the hero's buttons.

import { Crop, ListTodo, MoreHorizontal } from "lucide-react";
import { useState } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useDismiss } from "../../components/bits";
import { runEdit } from "../media/shared";
import type { Profile } from "../person/types";
import { noteText } from "./todoText";

/** Not in archives opened together: they are only browsed. */
const useTodoAllowed = () => useStore((s) => !s.archive?.combined);

function setTodo(id: string, todo: boolean, message: string) {
  useStore.getState().outsideSection(() =>
    runEdit(async () => {
      await call("person.setTodo", { id, todo });
      afterChange();
      useStore.getState().notify(message);
    }),
  );
}

/** „Do uzupełnienia”: why the person is on the list, „Gotowe” and (outside the family) „Dodaj relację”. Soft, not an alarm. */
export function TodoNote({ data }: { data: Profile }) {
  const allowed = useTodoAllowed();
  const go = useStore((s) => s.go);
  const requireEdit = useStore((s) => s.requireEdit);
  const todo = data.todo;
  if (!allowed || !todo?.unfinished) return null;
  const id = data.person.id;
  return (
    <section className="todo-note">
      <ListTodo size={18} color="var(--accent-text)" style={{ flex: "none", marginTop: 1 }} />
      <div className="col grow" style={{ gap: 2, minWidth: 0 }}>
        <b style={{ fontSize: 15 }}>Do uzupełnienia</b>
        <span style={{ color: "var(--text2)" }}>{noteText(todo)}</span>
      </div>
      <div className="row" style={{ gap: 6, flex: "none" }}>
        {!todo.linked && (
          <button className="btn secondary sm" onClick={() => requireEdit(() => go({ name: "person", id, section: "family", open: "family" }))}>
            Dodaj relację
          </button>
        )}
        <button className="btn secondary sm" onClick={() => setTodo(id, false, "Gotowe — usunięto z „Do uzupełnienia”.")}>
          Gotowe
        </button>
      </div>
    </section>
  );
}

/** The „⋯” menu at the end of the hero's buttons: a small list, more items can follow. */
export function ProfileMenu({ data, onCrop }: { data: Profile; onCrop?: (() => void) | null }) {
  const allowed = useTodoAllowed();
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  if (!allowed) return null;
  const id = data.person.id;
  const unfinished = !!data.todo?.unfinished;
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="icon-btn" aria-label="Więcej" title="Więcej" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <MoreHorizontal size={17} />
      </button>
      {open && (
        <div className="popover" role="menu" style={{ left: 0, top: 36, minWidth: 260, padding: "4px 0" }}>
          <button
            className="menu-item"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              if (unfinished) setTodo(id, false, "Gotowe — usunięto z „Do uzupełnienia”.");
              else setTodo(id, true, "Dodano do „Do uzupełnienia”.");
            }}
          >
            <ListTodo size={14} />
            {unfinished ? "Gotowe — usuń z „Do uzupełnienia”" : "Do dokończenia"}
          </button>
          {onCrop && (
            <button
              className="menu-item"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onCrop();
              }}
            >
              <Crop size={14} />
              Kadruj zdjęcie profilowe
            </button>
          )}
        </div>
      )}
    </div>
  );
}
