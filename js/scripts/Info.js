import React from 'react'

export default class Alert extends React.Component {
    render() {
        return this.props.info ? (
            <div className="kp-login-alert kp-login-alert-info" role="status">
                {this.props.info}
            </div>
        ) : null
    }
}
