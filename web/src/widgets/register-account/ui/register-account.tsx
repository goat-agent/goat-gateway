import { useCallback, useState, type ReactNode } from "react";
import { ArrowLeft, Boxes, ChevronRight, KeyRound, LogIn, Sparkles } from "lucide-react";
import { Button, ButtonLink, Dialog, Field, Input, SegmentedControl, toast, type Choice } from "@/shared/ui";
import { useResource } from "@/shared/api";
import type { ProviderEntry, Providers, SignInProvider } from "@/entities/provider";
import { useMe, type User } from "@/entities/user";
import { KeyForm, Owners, SetupTokenForm } from "@/features/add-account";
import { SignIn } from "@/features/oauth-signin";

type Step =
  | { kind: "choose" }
  | { kind: "plan"; provider: ProviderEntry }
  | { kind: "key"; provider: ProviderEntry };

const CLAUDE_WAYS = [
  { value: "token", label: "Paste a setup token" },
  { value: "browser", label: "Sign in in the browser" },
] as const;

type ClaudeWay = (typeof CLAUDE_WAYS)[number]["value"];

const PLAN_ORDER = ["chatgpt_signin", "anthropic", "chatgpt"];

const PLAN_CARDS: Record<string, { title: string; says: string }> = {
  chatgpt_signin: { title: "ChatGPT", says: "Sign in with ChatGPT" },
  anthropic: { title: "Claude Pro/Max", says: "Setup token or sign-in" },
  chatgpt: { title: "ChatGPT via Codex", says: "Codex's own sign-in" },
};

function planRank(id: string) {
  const at = PLAN_ORDER.indexOf(id);
  return at === -1 ? PLAN_ORDER.length : at;
}

function planCard(provider: ProviderEntry) {
  return (
    PLAN_CARDS[provider.id] ?? {
      title: provider.sign_in?.label ?? provider.label,
      says: provider.sign_in?.modes.join(", ") ?? "",
    }
  );
}

const TERMS: Record<string, string> = {
  anthropic:
    "Your plan serves your own requests. Requests from programs other than Claude Code reach Anthropic unchanged; Anthropic may serve them on the plan, bill them as extra usage, or refuse them. Turn extra usage off at claude.ai to get a refusal instead of a charge.",
  chatgpt:
    "Codex's own sign-in. It serves your own requests; OpenAI's terms do not let a plan be shared, so it is never offered to anyone else.",
  chatgpt_signin:
    "OpenAI's sign-in for apps. You can cap how much of your plan this gateway uses in your ChatGPT settings.",
};

export function RegisterAccount({
  open,
  onClose,
  onAdded,
}: {
  open: boolean;
  onClose: () => void;
  onAdded: () => void;
}) {
  const me = useMe((store) => store.me);
  const admin = me?.admin ?? false;
  const listed = useResource<Providers>(open ? "/api/providers" : undefined);
  const flows = useResource<{ providers: SignInProvider[] }>(open ? "/api/signin/providers" : undefined);
  const users = useResource<{ users: User[] }>(open && admin ? "/api/users" : undefined);

  const [step, setStep] = useState<Step>({ kind: "choose" });
  const [claudeWay, setClaudeWay] = useState<ClaudeWay>("token");
  const [name, setName] = useState("");
  const [owner, setOwner] = useState("");

  const providers = listed.data?.providers ?? [];
  const plans = providers
    .filter((provider) => provider.sign_in !== null)
    .sort((a, b) => planRank(a.id) - planRank(b.id));
  const keyed = providers.filter((provider) => provider.api_key);

  const people: Choice<string>[] = (users.data?.users ?? []).map((person) => ({
    value: person.id,
    label: person.id === me?.person.id ? `${person.name} (you)` : person.name,
  }));
  const yoursFirst = [...people].sort((a, b) =>
    a.value === me?.person.id ? -1 : b.value === me?.person.id ? 1 : 0,
  );
  const together = admin && people.length > 1;
  const planOwners = together ? yoursFirst : [];
  const keyOwners: Choice<string>[] = together ? [{ value: "everyone", label: "Everyone" }, ...yoursFirst] : [];
  const whose = owner || planOwners[0]?.value || "";

  const reset = () => {
    setStep({ kind: "choose" });
    setClaudeWay("token");
    setName("");
    setOwner("");
  };
  const close = () => {
    reset();
    onClose();
  };
  const added = useCallback(
    (called: string) => {
      toast(`Added ${called}`, { tone: "good", id: `added/${called}` });
      reset();
      onAdded();
    },
    [onAdded],
  );

  const back = (
    <Button size="medium" tone="ghost" onClick={() => setStep({ kind: "choose" })}>
      <ArrowLeft strokeWidth={1.75} />
      Back
    </Button>
  );

  const title =
    step.kind === "choose"
      ? "Add an account"
      : step.kind === "plan"
        ? planCard(step.provider).title
        : `${step.provider.label} API key`;

  return (
    <Dialog
      open={open}
      onClose={close}
      width={step.kind === "choose" ? "wide" : "medium"}
      title={title}
      description={
        step.kind === "choose"
          ? "A plan you pay for monthly, an API key, or an endpoint of your own."
          : step.kind === "plan"
            ? TERMS[step.provider.id]
            : undefined
      }
    >
      {step.kind === "choose" ? (
        <div className="flex flex-col gap-5">
          <Section title="Plans" icon={Sparkles}>
            {plans.map((provider) => (
              <Card
                key={provider.id}
                title={planCard(provider).title}
                says={planCard(provider).says}
                onPick={() => {
                  setName(planCard(provider).title);
                  setStep({ kind: "plan", provider });
                }}
              />
            ))}
          </Section>
          <Section title="API keys" icon={KeyRound}>
            {keyed.map((provider) => (
              <Card
                key={provider.id}
                title={provider.label}
                says={provider.endpoints.map((endpoint) => endpoint.wire).join(" · ")}
                onPick={() => setStep({ kind: "key", provider })}
              />
            ))}
          </Section>
          {admin ? (
          <div className="flex items-center justify-between gap-3 rounded-lg hairline border-line bg-base px-3 py-2.5">
            <span className="flex items-center gap-2 text-small text-ink-secondary">
              <Boxes className="size-4 text-ink-muted" strokeWidth={1.75} />
              An endpoint of your own, a proxy, or a local model server
            </span>
            <ButtonLink to="/providers?declare=1" size="small" onClick={close}>
              Declare a provider
            </ButtonLink>
          </div>
          ) : null}
        </div>
      ) : step.kind === "key" ? (
        <KeyForm provider={step.provider.id} label={step.provider.label} owners={keyOwners} onAdded={added}>
          {back}
        </KeyForm>
      ) : step.provider.id === "anthropic" && claudeWay === "token" ? (
        <div className="flex flex-col gap-4">
          <SegmentedControl label="How to add it" value={claudeWay} choices={CLAUDE_WAYS} onChange={setClaudeWay} />
          <SetupTokenForm label={name} owners={planOwners} onAdded={added}>
            {back}
          </SetupTokenForm>
        </div>
      ) : (
        <PlanSignIn
          provider={step.provider}
          name={name}
          onName={setName}
          owners={planOwners}
          owner={whose}
          onOwner={setOwner}
          offered={flows.data?.providers.find((flow) => flow.provider === step.provider.id)}
          onAdded={() => added(name.trim())}
          back={back}
          extra={
            step.provider.id === "anthropic" ? (
              <SegmentedControl label="How to add it" value={claudeWay} choices={CLAUDE_WAYS} onChange={setClaudeWay} />
            ) : null
          }
        />
      )}
    </Dialog>
  );
}

function PlanSignIn({
  provider,
  name,
  onName,
  owners,
  owner,
  onOwner,
  offered,
  onAdded,
  back,
  extra,
}: {
  provider: ProviderEntry;
  name: string;
  onName: (name: string) => void;
  owners: readonly Choice<string>[];
  owner: string;
  onOwner: (owner: string) => void;
  offered: SignInProvider | undefined;
  onAdded: () => void;
  back: ReactNode;
  extra: ReactNode;
}) {
  const [going, setGoing] = useState(false);

  if (going) {
    return (
      <SignIn
        provider={provider.id}
        name={name.trim()}
        owner={owners.length > 0 ? owner : undefined}
        offered={offered}
        onDone={onAdded}
        onCancel={() => setGoing(false)}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {extra}
      <Field label="Name" hint="What it is called in the console and in the request history.">
        <Input autoFocus value={name} onChange={(event) => onName(event.target.value)} />
      </Field>
      <Owners
        choices={owners}
        value={owner}
        onChange={onOwner}
        hint="A plan belongs to one person, and only their requests use it."
      />
      <div className="flex items-center justify-between gap-2 pt-1">
        {back}
        <Button tone="primary" size="medium" disabled={name.trim() === ""} onClick={() => setGoing(true)}>
          <LogIn strokeWidth={1.75} />
          Continue to sign-in
        </Button>
      </div>
    </div>
  );
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: typeof KeyRound;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="m-0 flex items-center gap-1.5 text-mini font-medium text-ink-secondary">
        <Icon className="size-3.5" strokeWidth={1.75} />
        {title}
      </h3>
      <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-3">{children}</div>
    </section>
  );
}

function Card({ title, says, onPick }: { title: string; says: string; onPick: () => void }) {
  return (
    <button
      type="button"
      onClick={onPick}
      className="hover-fade group flex min-w-0 items-center gap-2 rounded-lg hairline border-line bg-raised px-3 py-2.5 text-left shadow-low hover:border-line-solid hover:bg-raised-hover focus-visible:outline-1 focus-visible:outline-accent"
    >
      <span className="flex min-w-0 grow flex-col gap-0.5">
        <span className="truncate text-small font-medium text-ink-title">{title}</span>
        <span className="truncate text-mini text-ink-secondary">{says}</span>
      </span>
      <ChevronRight className="size-4 shrink-0 text-ink-muted group-hover:text-ink" strokeWidth={1.75} />
    </button>
  );
}
