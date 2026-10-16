import { useNavigate, useParams } from "react-router-dom";
import { AcceptInvite } from "@/features/accept-invite";

export function InvitePage() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  return <AcceptInvite token={token} onAccepted={() => navigate("/", { replace: true })} />;
}
