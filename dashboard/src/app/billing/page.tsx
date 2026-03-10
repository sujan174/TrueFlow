import { redirect } from "next/navigation";

// Billing is not yet available. Redirect to dashboard.
export default function BillingPage() {
    redirect("/");
}
