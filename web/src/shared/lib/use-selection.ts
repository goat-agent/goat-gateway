import { useCallback } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";

export function useSelection(base: string) {
  const { id } = useParams();
  const navigate = useNavigate();
  const { search } = useLocation();

  const select = useCallback(
    (next: string) => navigate({ pathname: `${base}/${encodeURIComponent(next)}`, search }),
    [base, navigate, search],
  );
  const close = useCallback(() => navigate({ pathname: base, search }), [base, navigate, search]);

  return { selected: id, select, close };
}
