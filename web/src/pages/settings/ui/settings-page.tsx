import type { ReactNode } from "react";
import { LogOut } from "lucide-react";
import { Button, Page, SegmentedControl } from "@/shared/ui";
import { useMe } from "@/entities/user";
import { useTheme, type Theme } from "@/features/toggle-theme";
import { signOut } from "@/features/sign-out";
import { ChangePassword } from "@/features/change-password";

const THEMES: readonly { value: Theme; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

export function SettingsPage() {
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);
  const me = useMe((store) => store.me);

  return (
    <Page title="Settings">
      <div className="mx-auto flex w-full max-w-[44rem] flex-col gap-10 px-6 pt-10 pb-16">
        <Group title="You">
          {me ? (
            <Setting
              name={me.person.name}
              says={
                me.admin
                  ? "An admin. You see and manage every person, account, and key."
                  : "A member. You see your own requests, keys, and plans, and the API keys shared with everyone."
              }
            />
          ) : null}
          <Setting
            name="Password"
            says="What you sign in to this console with. Forgotten, it is reset with the admin key on the sign-in screen."
            control={<ChangePassword />}
          />
        </Group>

        <Group title="Appearance">
          <Setting
            name="Theme"
            says="Kept in this browser only."
            control={
              <SegmentedControl
                label="Theme"
                value={theme}
                choices={THEMES}
                onChange={(picked) => {
                  if (picked !== theme) flip();
                }}
              />
            }
          />
        </Group>

        <Group title="This session">
          <Setting
            name="Sign out"
            says="Ends this session in this browser. Your password and keys do not change."
            control={
              <Button onClick={signOut}>
                <LogOut strokeWidth={1.75} />
                Sign out
              </Button>
            }
          />
        </Group>
      </div>
    </Page>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="display m-0 text-regular font-semibold text-ink-title">{title}</h2>
      <div className="flex flex-col rounded-xl hairline border-line bg-panel [&>*+*]:hairline-t [&>*+*]:border-line-subtle">
        {children}
      </div>
    </section>
  );
}

function Setting({ name, says, control }: { name: string; says: string; control?: ReactNode }) {
  return (
    <div className="flex min-h-16 items-center justify-between gap-6 px-4 py-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className="text-small font-medium text-ink">{name}</span>
        <span className="text-mini text-ink-secondary">{says}</span>
      </div>
      {control ? <div className="shrink-0">{control}</div> : null}
    </div>
  );
}
