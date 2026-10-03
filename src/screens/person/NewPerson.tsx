import { Info } from "lucide-react";
import { useEffect } from "react";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { PersonalEditor, type RelationKind } from "./PersonalSection";
import type { Profile } from "./types";
import "./profile.css";

/** A new person (design 17a): the profile's „Dane osobowe” section, open and empty. „Utwórz osobę” adds them (an
 *  unsaved change) and opens their profile, where the rest is added section by section. */
export function NewPerson({ relation }: { relation?: { kind: RelationKind; of: string } }) {
  const { data: relatedTo } = useApi<Profile>(relation ? "person.get" : null, { id: relation?.of });
  const mode = useStore((s) => s.mode);
  const requireEdit = useStore((s) => s.requireEdit);
  const setCrumb = useStore((s) => s.setCrumb);
  const go = useStore((s) => s.go);
  const goBack = useStore((s) => s.goBack);
  const notify = useStore((s) => s.notify);

  useEffect(() => {
    if (mode !== "edit") requireEdit();
  }, [mode, requireEdit]);
  useEffect(() => setCrumb("Nowa osoba"), [setCrumb]);

  if (relation && !relatedTo) return <div className="page" />;
  const related = relatedTo?.person;
  // A child takes the family's surname, a sibling the surname they were born with.
  const surname = !related ? undefined : relation?.kind === "child" ? related.surname : relation?.kind === "sibling" ? (related.maiden ?? related.surname) : undefined;

  return (
    <div className="page">
      <PersonalEditor
        id={null}
        profile={null}
        relation={relation}
        relatedName={related?.name}
        defaults={{ surname }}
        onDone={(created) => {
          if (!created) return;
          go({ name: "person", id: created });
          notify("Dodano osobę — zmiana czeka na zapis.", { detail: "Resztę dodasz na jej profilu, sekcja po sekcji." });
        }}
        onCancel={() => (relation ? go({ name: "person", id: relation.of }) : goBack())}
      />
      <div className="profile-body">
        <span />
        <div className="info-box row" style={{ gap: 10, alignItems: "flex-start" }}>
          <Info size={16} color="var(--accent-text)" style={{ flex: "none", marginTop: 2 }} />
          <span>Po utworzeniu osoby na jej profilu dodasz rodzinę, życiorys, historie, zdjęcia i linki — każdą sekcję osobno, przyciskiem „Edytuj sekcję”.</span>
        </div>
      </div>
    </div>
  );
}
