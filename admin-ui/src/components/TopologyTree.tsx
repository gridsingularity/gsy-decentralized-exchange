// Renders a community's topology from a market's `community_areas`.
//
// Data-model note: `community_areas` is a FLAT list with no parent/child
// linkage in the schema, and `area_uuid`/`area_hash` are randomized per market
// — only `name` + `area_type` are stable across slots. So we key React list
// items on `name` and present AREA-type nodes as containers with the remaining
// asset types (BATTERY/PV/EV/SMART_METER/GRID_METER/HEAT_PUMP/BOILER) grouped
// as leaf assets beneath them.

import type { AreaTopologySchema, AssetType } from '../api/schema';

const ASSET_META: Record<AssetType, { icon: string; label: string }> = {
  AREA: { icon: '🏘️', label: 'AREA' },
  BATTERY: { icon: '🔋', label: 'BAT' },
  PV: { icon: '☀️', label: 'PV' },
  EV: { icon: '🚗', label: 'EV' },
  SMART_METER: { icon: '📟', label: 'SM' },
  GRID_METER: { icon: '🔌', label: 'GRID' },
  HEAT_PUMP: { icon: '🌡️', label: 'HP' },
  BOILER: { icon: '♨️', label: 'BOIL' },
};

function meta(type: AssetType) {
  return ASSET_META[type] ?? { icon: '❔', label: String(type) };
}

interface Props {
  areas: AreaTopologySchema[];
  /** Selecting an asset node calls this with the asset's stable `name`. */
  onSelectAsset?: (name: string) => void;
  /** Highlight the active asset (matched by `name`). */
  selectedAsset?: string;
}

export default function TopologyTree({
  areas,
  onSelectAsset,
  selectedAsset,
}: Props) {
  if (areas.length === 0) {
    return <p className="muted">No areas in this market.</p>;
  }

  const containers = areas.filter((a) => a.area_type === 'AREA');
  const leaves = areas.filter((a) => a.area_type !== 'AREA');

  return (
    <ul className="topo-tree">
      {containers.map((c) => (
        <li key={c.name} className="topo-node topo-container">
          <AssetLabel
            node={c}
            onSelectAsset={onSelectAsset}
            selectedAsset={selectedAsset}
          />
        </li>
      ))}
      {leaves.length > 0 && (
        <li className="topo-node topo-container">
          <span className="topo-container-title">
            {containers.length === 0 ? '🏘️ Assets' : '↳ Assets'}
          </span>
          <ul className="topo-leaves">
            {leaves.map((leaf) => (
              <li key={leaf.name} className="topo-node topo-leaf">
                <AssetLabel
                  node={leaf}
                  onSelectAsset={onSelectAsset}
                  selectedAsset={selectedAsset}
                />
              </li>
            ))}
          </ul>
        </li>
      )}
    </ul>
  );
}

function AssetLabel({
  node,
  onSelectAsset,
  selectedAsset,
}: {
  node: AreaTopologySchema;
  onSelectAsset?: (name: string) => void;
  selectedAsset?: string;
}) {
  const m = meta(node.area_type);
  const isSelected = node.name === selectedAsset;
  const inner = (
    <>
      <span className="topo-icon">{m.icon}</span>
      <span className="topo-badge">{m.label}</span>
      <span className="topo-name">{node.name}</span>
    </>
  );

  if (!onSelectAsset) {
    return (
      <span className="topo-label" title={`${node.area_type} • ${node.name}`}>
        {inner}
      </span>
    );
  }

  return (
    <button
      type="button"
      className={isSelected ? 'topo-label topo-btn selected' : 'topo-label topo-btn'}
      title={`${node.area_type} • ${node.name}`}
      onClick={() => onSelectAsset(node.name)}
    >
      {inner}
    </button>
  );
}
