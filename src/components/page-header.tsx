import { Fragment, type ReactNode } from "react"
import { Link } from "react-router"

import { HistoryButtons } from "@/components/history-buttons"
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { textProps } from "@/lib/languages"

/** A crumb. `lang` is the language of a name in it (slunk's 3_alternate_names.md §7.1). */
export type Crumb = { label: string; to?: string; lang?: string | null }

/**
 * A page's header (slunk's design §7.1): back and forward (design §7.1) and
 * its breadcrumb title on the left, and its buttons and tabs (`children`) on
 * the right.
 */
export function PageHeader({
  crumbs,
  children,
}: {
  crumbs: Crumb[]
  children?: ReactNode
}) {
  return (
    <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
      <SidebarTrigger className="-ml-1" />
      <HistoryButtons />
      <Separator
        orientation="vertical"
        className="mr-2 data-vertical:h-4 data-vertical:self-auto"
      />
      <Breadcrumb>
        <BreadcrumbList>
          {crumbs.map((crumb, index) => (
            <Fragment key={index}>
              {index > 0 && <BreadcrumbSeparator />}
              <BreadcrumbItem>
                {crumb.to && index < crumbs.length - 1 ? (
                  <BreadcrumbLink render={<Link to={crumb.to} />}>
                    <span {...textProps(crumb.lang)}>{crumb.label}</span>
                  </BreadcrumbLink>
                ) : (
                  <BreadcrumbPage>
                    <span {...textProps(crumb.lang)}>{crumb.label}</span>
                  </BreadcrumbPage>
                )}
              </BreadcrumbItem>
            </Fragment>
          ))}
        </BreadcrumbList>
      </Breadcrumb>
      {children && (
        <div className="ml-auto flex items-center gap-2">{children}</div>
      )}
    </header>
  )
}
