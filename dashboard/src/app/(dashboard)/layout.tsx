import { createClient } from "@/lib/supabase/server";
import { redirect } from "next/navigation";
import { cookies } from "next/headers";
import { Sidebar } from "@/components/layout/sidebar";
import { Header } from "@/components/layout/header";
import { ProjectProviderWrapper } from "@/components/layout/project-provider-wrapper";
import { PermissionsProvider } from "@/contexts/permissions-context";

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const supabase = await createClient();
  const {
    data: { session },
  } = await supabase.auth.getSession();

  if (!session) {
    redirect("/login");
  }

  const {
    data: { user },
  } = await supabase.auth.getUser();

  // Read last_project_id from cookie (set during auth callback)
  const cookieStore = await cookies();
  const lastProjectId = cookieStore.get("trueflow_last_project")?.value || null;

  return (
    <ProjectProviderWrapper initialProjectId={lastProjectId}>
      <PermissionsProvider>
        <div className="flex h-screen bg-background">
          <Sidebar />
          <div className="flex-1 flex flex-col min-w-0">
            <Header user={user} />
            <main className="flex-1 overflow-auto">
              {children}
            </main>
          </div>
        </div>
      </PermissionsProvider>
    </ProjectProviderWrapper>
  );
}