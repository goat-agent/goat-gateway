import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { tinykeys, type KeybindingsMap } from "tinykeys";
import { useCommands } from "@/shared/model/commands";
import { typingOrOverlaid } from "@/shared/lib/keys";
import { usePlaces } from "./places";
import { useSheet } from "./sheet";

export function useShortcuts({ onRepeat }: { onRepeat: () => void }) {
  const navigate = useNavigate();
  const places = usePlaces();

  useEffect(() => {
    const going: KeybindingsMap = {};
    for (const place of places) {
      const go = (event: KeyboardEvent) => {
        event.preventDefault();
        navigate(place.to);
      };
      going[`g ${place.letter.toLowerCase()}`] = go;
      going[`KeyG Key${place.letter}`] = go;
    }

    const quiet = tinykeys(
      window,
      {
        ...going,
        "[Shift]+?": (event) => {
          event.preventDefault();
          useSheet.getState().setOpen(true);
        },
      },
      { ignore: typingOrOverlaid },
    );

    const loud = tinykeys(
      window,
      {
        "$mod+KeyK": (event) => {
          event.preventDefault();
          const commands = useCommands.getState();
          if (commands.open) onRepeat();
          else commands.show();
        },
      },
      { ignore: () => false },
    );

    return () => {
      quiet();
      loud();
    };
  }, [navigate, onRepeat, places]);
}
