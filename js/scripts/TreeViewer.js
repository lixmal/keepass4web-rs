import React from 'react'
import TreeNode from './TreeNode'

export default class TreeViewer extends React.Component {
    render() {
        var root = this.props.tree || {}
        var tree = root.children

        var children = []
        for (var i in tree) {
            children.push(
                <TreeNode
                    node={tree[i]}
                    level={1}
                    visible={true}
                    options={this.props}
                    key={tree[i].id}
                />
            )
        }

        var srcurl
        if (root.custom_icon_uuid) {
            srcurl = 'icon/' + encodeURIComponent(root.custom_icon_uuid)
        } else {
            srcurl = 'assets/img/icons/' + encodeURIComponent(root.icon || this.props.nodeIcon || '49') + '.png'
        }
        var nodeIcon = (
            <img src={srcurl} className="kp-icon icon"/>
        )

        return (
            <div className="panel panel-default">
                <div
                    className="treeview-header panel-heading"
                    onClick={this.props.nodeClick.bind(this, root)}
                >
                    {nodeIcon}
                    {root.title}
                </div>

                <ul className="treeview-body list-group">
                    {children}
                </ul>
            </div>
        )
    }
}


