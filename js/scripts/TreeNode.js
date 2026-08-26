import React from 'react'
import withNavigateHook from './nagivateHook'
import { IconFolder, IconChevDown, IconChevRight } from './Icons'

class TreeNode extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            expanded: props.node.expanded !== undefined
                ? props.node.expanded
                : props.level < (props.options.levels || 3),
        }
    }

    toggle(e) {
        this.setState(p => ({ expanded: !p.expanded }))
        e.stopPropagation()
    }

    select(node, e) {
        e.stopPropagation()
        const { nodeClick } = this.props.options
        if (nodeClick) nodeClick(node)
    }

    render() {
        const { node, level, visible, options } = this.props
        const { expanded } = this.state

        if (!visible) return null

        const hasChildren = node.children && node.children.length > 0

        let expandIcon = null
        if (hasChildren) {
            expandIcon = (
                <span
                    className="kp-tree-expand"
                    onClick={this.toggle.bind(this)}
                    style={{ display: 'flex', cursor: 'pointer', flexShrink: 0 }}
                >
                    {expanded ? <IconChevDown size={12}/> : <IconChevRight size={12}/>}
                </span>
            )
        } else {
            expandIcon = <span style={{ width: 12, flexShrink: 0 }}/>
        }

        let nodeIcon = null
        if (node.custom_icon_uuid) {
            nodeIcon = (
                <img
                    className="kp-icon"
                    style={{ width: 15, height: 15, objectFit: 'contain', flexShrink: 0 }}
                    src={'api/v1/icon/' + encodeURIComponent(node.custom_icon_uuid)}
                    alt=""
                />
            )
        } else {
            nodeIcon = (
                <span className="kp-tree-icon" style={{ display: 'flex' }}>
                    <IconFolder size={15}/>
                </span>
            )
        }

        const children = hasChildren ? node.children.map(child => (
            <TreeNode
                node={child}
                level={level + 1}
                visible={expanded}
                options={options}
                key={child.id}
            />
        )) : null

        return (
            <div>
                <div
                    className="kp-tree-item"
                    data-testid="tree-node"
                    onClick={this.select.bind(this, node)}
                >
                    {expandIcon}
                    {nodeIcon}
                    {node.title}
                </div>
                {children && (
                    <div className="kp-tree-children">
                        {children}
                    </div>
                )}
            </div>
        )
    }
}

export default withNavigateHook(TreeNode)
