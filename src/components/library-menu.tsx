import {
  ChevronsUpDownIcon,
  FolderOpenIcon,
  LibraryBigIcon,
  RepeatIcon,
} from "lucide-react"
import { useNavigate } from "react-router"
import { toast } from "sonner"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { closeLibrary, openLibrary } from "@/lib/library"
import { call, useCommand } from "@/lib/query"

/**
 * The sidebar's header (design §6.4): the open library's folder name, with a
 * menu to switch libraries, show it in Finder, or open a recent one.
 */
export function LibraryMenu() {
  const navigate = useNavigate()
  const { data: status } = useCommand("libraryStatus")
  const { data: recent } = useCommand("recentLibraries")
  const library = status?.library
  const others = (recent ?? []).filter(({ path }) => path !== library?.path)

  async function switchLibrary() {
    await closeLibrary()
    navigate("/library")
  }

  async function openRecent(path: string, name: string) {
    const id = toast.loading(`Opening ${name}…`)
    try {
      await openLibrary(path, () =>
        toast.loading("Backing up and upgrading the library…", { id })
      )
      toast.dismiss(id)
      navigate("/")
    } catch (error) {
      toast.error(`Can't open ${name}`, {
        id,
        description: (error as Error).message,
      })
    }
  }

  async function reveal() {
    try {
      await call("revealLibrary")
    } catch (error) {
      toast.error("Can't show the library", {
        description: (error as Error).message,
      })
    }
  }

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={<SidebarMenuButton size="lg" title={library?.folder} />}
          >
            <LibraryBigIcon />
            <span className="flex min-w-0 flex-col">
              <span className="truncate font-heading text-sm font-semibold">
                {library?.name ?? "liquidslunk"}
              </span>
              <span className="truncate text-xs text-muted-foreground">
                liquidslunk
              </span>
            </span>
            <ChevronsUpDownIcon className="ml-auto" />
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-64">
            <DropdownMenuItem onClick={() => void switchLibrary()}>
              <RepeatIcon />
              Switch library…
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => void reveal()}>
              <FolderOpenIcon />
              Show in Finder
            </DropdownMenuItem>
            {others.length > 0 && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuLabel>Recent libraries</DropdownMenuLabel>
                  {others.map((other) => (
                    <DropdownMenuItem
                      key={other.path}
                      disabled={!other.found}
                      title={other.folder}
                      onClick={() => void openRecent(other.path, other.name)}
                    >
                      <LibraryBigIcon />
                      <span className="truncate">{other.name}</span>
                      {!other.found && (
                        <span className="ml-auto text-muted-foreground">
                          Not found
                        </span>
                      )}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuGroup>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}
