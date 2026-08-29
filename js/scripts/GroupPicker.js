import React from 'react'
import { IconFolder } from './Icons'

// Flattens the group tree into rows the picker can list, keeping the depth so
// the nesting is still visible in a flat list.
function flatten(group, depth = 0, into = []) {
    if (!group) return into

    into.push({ id: group.id, title: group.title, depth })
    for (const child of group.children || []) {
        flatten(child, depth + 1, into)
    }
    return into
}

// Picks a destination group. The group being moved is excluded, and so is
// everything under it: a group cannot be moved inside itself.
function excluded(group, excludeId, into = new Set()) {
    if (!group) return into
    if (group.id === excludeId) {
        flatten(group).forEach(row => into.add(row.id))
        return into
    }
    for (const child of group.children || []) {
        excluded(child, excludeId, into)
    }
    return into
}

export default function GroupPicker({ tree, title, currentId, excludeId, onPick, onCancel }) {
    const skip = excludeId ? excluded(tree, excludeId) : new Set()
    const rows = flatten(tree).filter(row => !skip.has(row.id))

    return (
        <div className="kp-modal-backdrop" onClick={onCancel}>
            <div className="kp-modal" data-testid="group-picker" onClick={ev => ev.stopPropagation()}>
                <h3>{title}</h3>
                <div className="kp-picker-list">
                    {rows.map(row => (
                        <button
                            key={row.id}
                            type="button"
                            className="kp-picker-row"
                            data-testid="group-picker-row"
                            disabled={row.id === currentId}
                            style={{ paddingLeft: 10 + row.depth * 16 }}
                            onClick={() => onPick(row.id)}
                        >
                            <IconFolder size={14}/>
                            {row.title}
                            {row.id === currentId && <span className="kp-picker-current">current</span>}
                        </button>
                    ))}
                </div>
                <div className="kp-modal-actions">
                    <button type="button" className="kp-btn kp-btn-ghost" onClick={onCancel}>Cancel</button>
                </div>
            </div>
        </div>
    )
}
