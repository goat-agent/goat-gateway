import type { ReactNode } from "react";
import { Boxes, LogOut } from "lucide-react";
import {
  Body,
  Button,
  ButtonLink,
  Cell,
  Count,
  Head,
  Nothing,
  Page,
  Panel,
  Row,
  SegmentedControl,
  Table,
  rowHover,
} from "@/shared/ui";
import { useResource } from "@/shared/api";
import { useMe } from "@/entities/user";
import { money } from "@/shared/lib/format";
import { useTheme, type Theme } from "@/features/toggle-theme";
import { signOut } from "@/features/sign-out";
import { ChangePassword } from "@/features/change-password";

type Price = {
  provider: string;
  model: string;
  input_per_mtok_micros: number;
  output_per_mtok_micros: number;
  cache_read_per_mtok_micros: number;
  cache_write_per_mtok_micros: number;
};

const THEMES: readonly { value: Theme; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

export function SettingsPage() {
  const pricing = useResource<{ prices: Price[]; as_of: string }>("/api/pricing");
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);
  const me = useMe((store) => store.me);
  const person = me?.person ?? null;
  const prices = pricing.data?.prices ?? [];

  return (
    <Page title="Settings" refused={pricing.error} bleed>
      <div className="mx-auto flex w-full max-w-[52rem] flex-col gap-10 px-6 py-8">
        <Section title="You">
          <Panel>
            {person ? (
              <>
                <Setting
                  name={person.name}
                  says={
                    person.role === "admin"
                      ? "An admin. You see and manage every person, account, and key."
                      : "A member. You see your own requests, keys, and plans, and the API keys shared with everyone."
                  }
                  control={null}
                />
                <div className="hairline-t border-line-subtle">
                  <Setting
                    name="Password"
                    says="What you sign in to this console with."
                    control={<ChangePassword />}
                  />
                </div>
              </>
            ) : (
              <Setting
                name="The admin key"
                says="This session answers for everyone. Say who you are in the banner above to sign in as a person from now on."
                control={null}
              />
            )}
          </Panel>
        </Section>

        <Section
          title="Prices"
          says={
            pricing.data
              ? `As of ${pricing.data.as_of}, per million tokens. A model without a price reports no cost.`
              : "Per million tokens. A model without a price reports no cost."
          }
        >
          <Panel>
            {prices.length === 0 ? (
              <Nothing says="No model declares a price. Requests show no cost rather than a cost of zero." />
            ) : (
              <Table className="[&_thead_th]:border-t-0">
                <Head>
                  <Row>
                    <th>Model</th>
                    <Count>Input</Count>
                    <Count>Output</Count>
                    <Count>Cache read</Count>
                    <Count>Cache write</Count>
                  </Row>
                </Head>
                <Body>
                  {prices.map((price) => (
                    <Row key={`${price.provider}-${price.model}`} className={rowHover}>
                      <td className="font-medium text-ink">{price.model}</td>
                      <Cell>{money(price.input_per_mtok_micros)}</Cell>
                      <Cell>{money(price.output_per_mtok_micros)}</Cell>
                      <Cell>{money(price.cache_read_per_mtok_micros)}</Cell>
                      <Cell>{money(price.cache_write_per_mtok_micros)}</Cell>
                    </Row>
                  ))}
                </Body>
              </Table>
            )}
          </Panel>
        </Section>

        <Section title="Providers">
          <Panel>
            <Setting
              name="What this gateway reaches"
              says="Built-in declarations and your own in config.toml, the formats each speaks, and the models each declares."
              control={
                <ButtonLink to="/providers">
                  <Boxes strokeWidth={1.75} />
                  Open providers
                </ButtonLink>
              }
            />
          </Panel>
        </Section>

        <Section title="Appearance">
          <Panel>
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
          </Panel>
        </Section>

        <Section title="This session">
          <Panel>
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
          </Panel>
        </Section>
      </div>
    </Page>
  );
}

function Section({
  title,
  says,
  children,
}: {
  title: string;
  says?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <h2 className="display m-0 text-regular font-semibold text-ink-title">{title}</h2>
        {says ? <p className="m-0 text-small text-ink-secondary">{says}</p> : null}
      </div>
      {children}
    </section>
  );
}

function Setting({
  name,
  says,
  control,
}: {
  name: string;
  says: string;
  control: ReactNode;
}) {
  return (
    <div className="flex min-h-16 items-center justify-between gap-6 px-4 py-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className="text-small font-medium text-ink">{name}</span>
        <span className="text-mini text-ink-secondary">{says}</span>
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}
