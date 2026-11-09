import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, GitBranch } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { openVault } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import { Button, cn, toast } from "../../ui";
import { CloneVaultCard, CreateVaultCard } from "./cards";
import { AuthStage, GiantTitle, RisingCard, StageNote, StageTopBar } from "./stage";

type Card = "create" | "clone";

/** Welcome ↔ open scene crossfade; the card sits on top of either. */
export function Welcome(): ReactElement {
  const m = useMessages();
  const [scene, setScene] = useState<"open" | undefined>(undefined);
  const [card, setCard] = useState<Card | undefined>(undefined);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && card === undefined && scene !== undefined) {
        setScene(undefined);
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  }, [card, scene]);

  const pickFolder = async (): Promise<void> => {
    const dir = await pickDialog({ directory: true });
    if (typeof dir !== "string") {
      return;
    }
    try {
      await openVault(dir);
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  return (
    <AuthStage>
      <div className="relative flex-1">
        <div
          className={cn(
            "ease-standard absolute inset-0 flex flex-col transition-opacity duration-260",
            scene !== undefined && "pointer-events-none opacity-0",
          )}
          inert={scene !== undefined}
        >
          <StageTopBar />
          <GiantTitle lines={m.welcome.title} />
          <StageNote>{m.welcome.sub}</StageNote>
          <div className="flex-1" />
          <div className="flex gap-2">
            <Button
              variant="pill-strong"
              size="xl"
              block
              onClick={() => {
                setCard("create");
              }}
            >
              {m.welcome.newVault}
            </Button>
            <Button
              variant="pill"
              size="xl"
              block
              onClick={() => {
                setScene("open");
              }}
            >
              {m.welcome.haveOne}
            </Button>
          </div>
          <p className="text-muted pt-4 text-center text-xs">{m.welcome.note}</p>
        </div>
        <div
          className={cn(
            "ease-standard absolute inset-0 flex flex-col transition-opacity duration-240",
            scene === undefined && "pointer-events-none opacity-0",
          )}
          inert={scene === undefined}
        >
          <StageTopBar
            onBack={() => {
              setScene(undefined);
            }}
          />
          <GiantTitle lines={[m.openScene.title]} />
          <div className="flex-1" />
          <div className="flex flex-col gap-1.5">
            <Button
              variant="pill"
              size="xl"
              block
              align="start"
              icon={FolderOpen}
              onClick={() => void pickFolder()}
            >
              {m.openScene.chooseFolder}
            </Button>
            <Button
              variant="pill"
              size="xl"
              block
              align="start"
              icon={GitBranch}
              onClick={() => {
                setCard("clone");
              }}
            >
              {m.openScene.clone}
            </Button>
          </div>
        </div>
      </div>
      <RisingCard
        open={card === "create"}
        onOpenChange={(open) => {
          if (!open) {
            setCard(undefined);
          }
        }}
      >
        <CreateVaultCard />
      </RisingCard>
      <RisingCard
        open={card === "clone"}
        onOpenChange={(open) => {
          if (!open) {
            setCard(undefined);
          }
        }}
      >
        <CloneVaultCard />
      </RisingCard>
    </AuthStage>
  );
}
