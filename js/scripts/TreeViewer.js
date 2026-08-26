import React from 'react'
import TreeNode from './TreeNode'
import withNavigateHook from './nagivateHook'
import { IconFolderRoot, IconPlus } from './Icons'

class TreeViewer extends React.Component {
    render() {
        const root  = this.props.tree || {}
        const nodes = root.children || []

        const children = nodes.map(n => (
            <TreeNode
                node={n}
                level={1}
                visible
                options={this.props}
                key={n.id}
            />
        ))

        const folderCount = nodes.filter(n => n.children !== undefined).length || nodes.length

        return (
            <aside className={`kp-sidebar${this.props.open ? ' open' : ''}`} id="kp-sidebar">
                <div className="kp-sidebar-header">
                    <div>
                        <div className="kp-sidebar-label">Groups</div>
                        <div className="kp-sidebar-count">{folderCount} folder{folderCount !== 1 ? 's' : ''}</div>
                    </div>
                    {this.props.onAddRootGroup && (
                        <button
                            className="kp-sidebar-add"
                            title="Add group to root"
                            onClick={this.props.onAddRootGroup}
                        >
                            <IconPlus size={13}/>
                        </button>
                    )}
                </div>

                <div className="kp-tree">
                    {/* Root row */}
                    <div
                        className="kp-tree-item"
                        data-testid="tree-root"
                        onClick={() => this.props.nodeClick && this.props.nodeClick(root)}
                    >
                        <span style={{ width: 12, flexShrink: 0 }}/>
                        <IconFolderRoot size={15}/>
                        {root.title || 'Root'}
                    </div>

                    <div className="kp-tree-children">
                        {children}
                    </div>
                </div>
            </aside>
        )
    }
}

export default withNavigateHook(TreeViewer)
