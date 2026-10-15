import { memo } from "react";
import { useBreakdown } from "../hooks/useBreakdown";
import type { Card } from "../types";
import StorageDetail from "./StorageDetail";

interface Props {
  cards: Card[] | null;
  onReclaim: (card: Card) => void;
  locked: boolean;
}

/**
 * Owns the storage detail state, so the walk's updates re-render this
 * section only, never the reclaim cards and overview around it.
 */
function StorageDetailPanel({ cards, onReclaim, locked }: Props) {
  const b = useBreakdown(cards);
  return (
    <StorageDetail
      screen={b.screen}
      onOpen={b.open}
      onUp={b.up}
      links={b.links}
      onReclaim={onReclaim}
      onFullDiskAccess={b.openFullDiskAccess}
      locked={locked}
    />
  );
}

export default memo(StorageDetailPanel);
