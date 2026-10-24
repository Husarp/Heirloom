// „Kadruj” on the profile photo: the hero's portrait, the „⋯” menu and the gallery's „Profilowe” spot share this.

import { Crop as CropIcon } from "lucide-react";
import { useState, type ReactNode } from "react";
import { call, mediaUrl } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { CropDialog } from "../../components/CropDialog";
import type { Crop } from "../../components/cropMath";
import { runEdit } from "../media/shared";
import type { Profile } from "./types";

/** `start` is null when there is nothing to crop or the profile cannot be edited here; `dialog` goes anywhere in the
 *  component's output. */
export function useProfileCrop(data: Profile): { start: (() => void) | null; dialog: ReactNode } {
  const [open, setOpen] = useState(false);
  const canEdit = useStore((s) => s.mode === "edit" && !(s.archive?.combined || s.archive?.readOnly));
  const portrait = data.portrait;
  // The gallery item of the profile photo has the media id the portrait lacks.
  const photoId = data.gallery.find((g) => g.profile)?.id;
  // An object written inside the person (id with `#`) has no record of its own to cut.
  if (!canEdit || !portrait || !photoId || photoId.includes("#")) return { start: null, dialog: null };

  const person = data.person.id;
  const save = (crop: Crop | null) => {
    setOpen(false);
    useStore.getState().outsideSection(() =>
      runEdit(async () => {
        await call("media.setCrop", { person, id: photoId, crop });
        afterChange();
        useStore.getState().notify(crop ? "Zapisano kadr zdjęcia profilowego." : "Zdjęcie profilowe bez kadrowania.");
      }),
    );
  };
  return {
    start: () => setOpen(true),
    dialog: open && <CropDialog src={mediaUrl(portrait.path, "view")} initial={portrait.crop} title="Kadr zdjęcia profilowego" onSave={save} onClose={() => setOpen(false)} />,
  };
}

/** The small „Kadruj” button in the corner of a photo. */
export function CropButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="crop-btn" onClick={onClick} title="Wybierz, co ma być widać w kółku i na profilu">
      <CropIcon size={13} />
      Kadruj
    </button>
  );
}
