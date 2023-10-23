import React from 'react'

import NavBar from "./NavBar"

import withNavigateHook from "./nagivateHook"

class Splash extends React.Component {
    componentDidMount() {
        // pass previous state to next route to display errors etc
        this.serverRequest = KeePass4Web.checkAuth.call(this, this.props.location.state)
    }

    componentWillUnmount() {
        if (this.serverRequest)
            this.serverRequest.abort()
    }

    render() {
        return (
            <div className="loading-mask">
                <NavBar/>
            </div>
        )
    }
}

export default withNavigateHook(Splash)
