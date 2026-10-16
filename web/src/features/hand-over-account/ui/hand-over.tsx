import { send } from "@/shared/api";
import { Select, toast, type Choice } from "@/shared/ui";
import type { Account } from "@/entities/account";
import type { User } from "@/entities/user";

const EVERYONE = "everyone";
const NOBODY = "";

export function HandOver({
  account,
  people,
  onChanged,
}: {
  account: Account;
  people: readonly User[];
  onChanged: () => void;
}) {
  const plan = account.credential_kind !== "api_key";
  const named: Choice<string>[] = people.map((person) => ({ value: person.id, label: person.name }));
  const whose = account.owner ?? (plan ? NOBODY : EVERYONE);
  const choices: Choice<string>[] = plan
    ? whose === NOBODY
      ? [{ value: NOBODY, label: "Nobody yet" }, ...named]
      : named
    : [{ value: EVERYONE, label: "Everyone" }, ...named];
  const nameOf = (owner: string) =>
    owner === EVERYONE ? "everyone" : (people.find((person) => person.id === owner)?.name ?? owner);

  return (
    <Select
      label={`Whose ${account.name} is`}
      value={whose}
      choices={choices}
      className="-ml-2.5 max-w-44 border-transparent bg-transparent shadow-none hover:border-line"
      onChange={(owner) => {
        if (owner === NOBODY || owner === whose) return;
        send(`/api/accounts/${encodeURIComponent(account.name)}/owner`, { owner })
          .then(() => {
            toast(`${account.name} now belongs to ${nameOf(owner)}`, {
              tone: "good",
              id: `owner/${account.name}`,
            });
            onChanged();
          })
          .catch((error: Error) =>
            toast(`Could not hand ${account.name} over`, { says: error.message, tone: "critical" }),
          );
      }}
    />
  );
}
