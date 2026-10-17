import { FileQuestionIcon } from "lucide-react"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"

/** Shown for an unknown route, and later for an unknown id (slunk's design §7.2). */
export function NotFound() {
  return (
    <>
      <PageHeader crumbs={[{ label: "Not found" }]} />
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <FileQuestionIcon />
          </EmptyMedia>
          <EmptyTitle>Not found</EmptyTitle>
          <EmptyDescription>There's nothing at this address.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <ButtonLink to="/">Go to the dashboard</ButtonLink>
        </EmptyContent>
      </Empty>
    </>
  )
}
