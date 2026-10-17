import { artistUrl, Names } from "@/components/names"
import type { CreditedLink } from "@/lib/credited"

type CreditLine = {
  role: string
  /** As credited: "J. Van Heusen" links to Jimmy Van Heusen */
  artists: CreditedLink[]
}

/** Credit lines, in order: "Engineer – Paul Barrett, Robbie Adams". */
export function CreditLines({ credits }: { credits: CreditLine[] }) {
  return (
    <div className="flex flex-col gap-0.5">
      {credits.map(({ role, artists }) => (
        // A role appears once per release or track
        <span key={role}>
          <span className="text-muted-foreground">{role}</span> –{" "}
          <Names items={artists} to={artistUrl} />
        </span>
      ))}
    </div>
  )
}
